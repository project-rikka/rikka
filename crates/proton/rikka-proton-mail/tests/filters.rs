use std::sync::{Arc, Mutex};

use rikka_proton_mail::{
    Comparator, Condition, Error, Filter, FilterId, SieveScript, SimpleFilter,
};
use serde_json::{Value, json};

use crate::support::signed_in;
use rikka_test_proton::{Request, json_response, not_found, response};

fn samples() -> Vec<SimpleFilter> {
    vec![
        SimpleFilter::all()
            .when(Condition::subject(
                Comparator::Contains,
                ["invoice", "receipt"],
            ))
            .file_into("Archive")
            .mark_read(),
        SimpleFilter::any()
            .when(Condition::sender(Comparator::Starts, ["boss"]))
            .when(Condition::recipient(Comparator::Ends, ["@team.test"]).negated())
            .when(Condition::has_attachments().negated())
            .file_into("Work/Reports")
            .file_into("Important")
            .mark_starred()
            .auto_reply("<p>I am away until \"Monday\"</p>"),
        SimpleFilter::all()
            .when(Condition::subject(
                Comparator::Starts,
                ["50% * off \\ now?"],
            ))
            .when(Condition::sender(Comparator::Is, ["deals@shop.test"]))
            .when(Condition::subject(Comparator::Matches, ["sale*week?"]))
            .file_into("Spam"),
    ]
}

#[test]
fn simple_filters_round_trip_sieve() {
    for filter in samples() {
        let script = filter.to_sieve();
        assert!(SieveScript::parse(&script).is_ok(), "{script}");
        assert_eq!(SimpleFilter::from_sieve(&script), Some(filter), "{script}");
    }
}

#[test]
fn simple_filter_serializes_like_proton() {
    let script = samples()[0].to_sieve();

    for expected in [
        r#"require ["include", "environment", "variables", "relational", "comparator-i;ascii-numeric", "spamtest"];"#,
        r#"require ["fileinto", "imap4flags"];"#,
        r#"if allof (environment :matches "vnd.proton.spam-threshold" "*","#,
        r#"if allof (header :comparator "i;unicode-casemap" :contains "Subject" ["invoice", "receipt"])"#,
        r#"    fileinto "Archive";"#,
        r#"    addflag "\\Seen";"#,
    ] {
        assert!(script.contains(expected), "{expected}\n---\n{script}");
    }
}

#[test]
fn sieve_errors_report_position() {
    let script = SieveScript::parse(
        "# comment\r\nrequire [\"vacation\"];\r\n/* block\r\ncomment */\r\n\
         vacation :days 7 text:\r\nline one\r\n..dotted\r\n.\r\n;\r\n",
    )
    .unwrap();
    let vacation = &script.commands()[1];
    assert_eq!(vacation.name(), "vacation");
    assert_eq!(
        vacation
            .arguments()
            .last()
            .and_then(|argument| argument.strings()),
        Some(vec!["line one\r\n.dotted\r\n"])
    );

    let broken = [
        ("if header :is \"Subject\" \"x\" { fileinto \"a\" }", 1, 44),
        ("require \"fileinto\";\nfileinto \"open", 2, 15),
        ("if anyof (true, ) { stop; }", 1, 17),
    ];
    for (source, line, column) in broken {
        let error = SieveScript::parse(source).expect_err(source);
        assert_eq!(
            (error.line(), error.column()),
            (line, column),
            "{source}: {error}"
        );
    }

    let advanced = [
        "require \"fileinto\";\nif size :over 100K { discard; }",
        "if header :is \"Subject\" \"a\" { fileinto \"x\"; } else { fileinto \"y\"; }",
        "if header :regex \"Subject\" \"^a\" { stop; }",
    ];
    for source in advanced {
        assert!(SimpleFilter::from_sieve(source).is_none(), "{source}");
    }
}

#[test]
fn listed_filter_exposes_simple_rule() {
    let simple = samples()[0].clone();
    let parsed_filter: Filter = serde_json::from_value(json!({
        "ID": "f1",
        "Name": "Invoices",
        "Status": 1,
        "Priority": 2,
        "Version": 2,
        "Sieve": simple.to_sieve(),
        "Simple": {
            "Operator": {"label": "All", "value": "all"},
            "Conditions": [{
                "Type": {"label": "Subject", "value": "subject"},
                "Comparator": {"label": "contains", "value": "contains"},
                "Values": ["invoice", "receipt"]
            }],
            "Actions": {"FileInto": ["Archive"], "Mark": {"Read": true, "Starred": false}, "Vacation": null}
        }
    }))
    .unwrap();

    assert!(parsed_filter.is_enabled());
    assert_eq!(parsed_filter.simple(), Some(simple));

    let advanced: Filter = serde_json::from_value(json!({
        "ID": "f2", "Name": "Big", "Status": 0, "Version": 2,
        "Sieve": "require \"fileinto\";\nif size :over 1M { fileinto \"Big\"; }"
    }))
    .unwrap();
    assert!(advanced.is_advanced());
    assert!(!advanced.is_enabled());
}

#[tokio::test(flavor = "multi_thread")]
async fn filter_management_endpoints() {
    let captured: Arc<Mutex<Vec<(String, String, Value)>>> = Arc::default();
    let noted = Arc::clone(&captured);

    let signed = signed_in(move |request: Request| {
        noted.lock().unwrap().push((
            request.method.clone(),
            request.path.clone(),
            request.json(),
        ));
        let filter = json!({"Code": 1000, "Filter": {"ID": "f1", "Name": "Invoices", "Status": 1, "Version": 2, "Sieve": ""}});
        match request.route() {
            ("GET", "/mail/v4/filters") => json_response(200, &json!({"Code": 1000, "Filters": [filter["Filter"]]})),
            ("POST", "/mail/v4/filters") | ("PUT", "/mail/v4/filters/f1") => json_response(200, &filter),
            ("PUT", "/mail/v4/filters/check") => json_response(
                200,
                &json!({"Code": 1000, "Issues": [{
                    "message": "unknown command",
                    "severity": "error",
                    "from": {"line": 2, "ch": 4},
                    "to": {"line": 2, "ch": 9}
                }]}),
            ),
            ("PUT", "/mail/v4/filters/f1/enable" | "/mail/v4/filters/f1/disable" | "/mail/v4/filters/order")
            | ("DELETE", "/mail/v4/filters/f1") => response(200, r#"{"Code":1000}"#),
            _ => not_found(),
        }
    })
    .await;
    let client = &signed.client;
    let id = FilterId::from("f1");

    assert_eq!(client.filters().await.unwrap().len(), 1);
    client
        .new_filter("Invoices")
        .simple(samples()[0].clone())
        .create()
        .await
        .unwrap();
    client
        .new_filter("Raw")
        .sieve("keep;")
        .enabled(false)
        .update(&id)
        .await
        .unwrap();
    client.set_filter_enabled(&id, false).await.unwrap();
    client
        .order_filters(&[FilterId::from("f2"), id.clone()])
        .await
        .unwrap();
    let issues = client.check_sieve("frobnicate;").await.unwrap();
    client.delete_filter(&id).await.unwrap();

    assert_eq!(issues[0].message(), "unknown command");
    assert!(issues[0].is_error());
    assert_eq!(
        issues[0].start().map(|at| (at.line(), at.column())),
        Some((2, 4))
    );

    let missing_rule = client.new_filter("Empty").create().await;
    assert!(matches!(missing_rule, Err(Error::MissingFilterRule)));
    let missing_name = client.new_filter("  ").sieve("keep;").create().await;
    assert!(matches!(missing_name, Err(Error::MissingFilterName)));

    let captured = captured.lock().unwrap();
    let request: Vec<(&str, &str)> = captured
        .iter()
        .filter(|(_, path, _)| path.starts_with("/mail/"))
        .map(|(method, path, _)| (method.as_str(), path.as_str()))
        .collect();
    assert_eq!(
        request,
        vec![
            ("GET", "/mail/v4/filters"),
            ("POST", "/mail/v4/filters"),
            ("PUT", "/mail/v4/filters/f1"),
            ("PUT", "/mail/v4/filters/f1/disable"),
            ("PUT", "/mail/v4/filters/order"),
            ("PUT", "/mail/v4/filters/check"),
            ("DELETE", "/mail/v4/filters/f1"),
        ]
    );

    let body = |at: usize| {
        &captured
            .iter()
            .filter(|(_, path, _)| path.starts_with("/mail/"))
            .nth(at)
            .unwrap()
            .2
    };
    let created = body(1);
    assert_eq!(created["Name"], "Invoices");
    assert_eq!(created["Status"], 1);
    assert_eq!(created["Version"], 2);
    assert_eq!(created["Sieve"], samples()[0].to_sieve());
    assert_eq!(created["Simple"]["Operator"]["value"], "all");
    assert_eq!(
        created["Simple"]["Conditions"][0]["Comparator"]["value"],
        "contains"
    );
    assert_eq!(created["Simple"]["Actions"]["Mark"]["Read"], true);

    let updated = body(2);
    assert_eq!(updated["Status"], 0);
    assert_eq!(updated["Sieve"], "keep;");
    assert!(updated.get("Simple").is_none());

    assert_eq!(body(4)["FilterIDs"], json!(["f2", "f1"]));
    assert_eq!(body(5), &json!({"Sieve": "frobnicate;", "Version": 2}));
}
