use browser_translate::{Error, Job, Node, Output, OutputNode, Translator, Unit};
use rikka_languages::Language;

use crate::support::{Fake, echo_upper, japanese, text, translated};

fn translator() -> Translator {
    Translator::new().expect("create translator")
}

#[tokio::test]
async fn units_round_trip_through_dialect() {
    let fake = Fake::new(echo_upper);
    let units = vec![
        text("Hello world"),
        Unit::new(vec![
            Node::Text("Hello ".into()),
            Node::Element {
                id: 0,
                nodes: vec![Node::Text("there".into())],
            },
        ])
        .expect("a valid unit"),
        text("   "),
    ];
    let translation = translator()
        .translate(&fake, Job::new(japanese(), units))
        .await
        .expect("translate");
    assert_eq!(
        translation.outputs,
        [
            Output {
                nodes: vec![OutputNode::Text("こんにちは world".into())]
            },
            Output {
                nodes: vec![
                    OutputNode::Text("こんにちは ".into()),
                    OutputNode::Element {
                        id: 0,
                        nodes: vec![OutputNode::Text("there".into())]
                    }
                ]
            },
            Output {
                nodes: vec![OutputNode::Text("   ".into())]
            },
        ]
    );
    assert_eq!(translation.detected, Language::lookup("en"));
    assert_eq!(
        fake.sent(),
        [(
            vec![
                "Hello world".to_owned(),
                "Hello <a i=0>there</a>".to_owned()
            ],
            "ja"
        )]
    );
}

#[tokio::test]
async fn requests_respect_service_limits() {
    let fake = Fake::new(echo_upper);
    let translator = translator();
    let short: Vec<Unit> = (0..10)
        .map(|index| text(&format!("line {index}")))
        .collect();
    translator
        .translate(&fake, Job::new(japanese(), short))
        .await
        .expect("translate");
    assert_eq!(fake.sizes(), [2, 4, 4]);
    let long: Vec<Unit> = (0..5)
        .map(|index| text(&format!("{index}{}", "x".repeat(24))))
        .collect();
    translator
        .translate(&fake, Job::new(japanese(), long).protect(false))
        .await
        .expect("translate");
    assert_eq!(fake.sizes(), [1, 2, 2]);
}

#[tokio::test]
async fn protected_identifiers_round_trip() {
    let fake = Fake::new(|_| {
        Ok(vec![translated(
            "<code i=0 translate=\"no\">src/main.rs</code>を開く".to_owned(),
            "en",
        )])
    });
    let translation = translator()
        .translate(&fake, Job::new(japanese(), vec![text("Open src/main.rs")]))
        .await
        .expect("translate");
    assert_eq!(
        translation.outputs,
        [Output {
            nodes: vec![OutputNode::Text("src/main.rsを開く".into())]
        }]
    );
    assert_eq!(
        fake.sent()[0].0,
        ["Open <code i=0 translate=\"no\">src/main.rs</code>"]
    );
}

#[tokio::test]
async fn elements_returned_exactly_once() {
    let element = |id: u32, nodes: Vec<Node>| Node::Element { id, nodes };
    let keep = |id: u32, text: &str| Node::Keep {
        id,
        text: text.to_owned(),
    };
    let source = Unit::new(vec![
        Node::Text("A ".into()),
        element(
            0,
            vec![
                Node::Text("b ".into()),
                element(1, vec![Node::Text("c".into())]),
                keep(2, "d"),
            ],
        ),
        element(3, vec![keep(4, "")]),
        Node::Text(" e_f".into()),
    ])
    .expect("a valid unit");
    let answers = [
        "",
        "garbage < > & ;",
        "<a i=1><a i=1><a i=0><a i=3><a i=4>x</a></a></a></a></a>",
        "<code i=2 translate=no><a i=0>q</a></code><a i=99>r</a><a i=-1>s</a><a i=x>t</a>",
        "<a i=4>1</a><a i=3>2</a><a i=2>3</a><a i=1>4</a><a i=0>5</a><code i=5>6</code><code i=5>7</code>",
        "</a></a><a i=0",
    ];
    for answer in answers {
        let fake = Fake::new(move |_| Ok(vec![translated(answer.to_owned(), "en")]));
        let translation = translator()
            .translate(&fake, Job::new(japanese(), vec![source.clone()]))
            .await
            .expect("translate");
        let decoded = &translation.outputs[0].nodes;
        let mut ids = Vec::new();
        collect(decoded, None, &mut ids);
        ids.sort_unstable();
        assert_eq!(
            ids,
            [
                (0, None),
                (1, Some(0)),
                (2, Some(0)),
                (3, None),
                (4, Some(3))
            ],
            "{answer}"
        );
        assert!(has_text_in(decoded, 0), "{answer}");
        assert!(has_text_in(decoded, 1), "{answer}");
        assert!(flatten(decoded).contains("e_f"), "{answer}");
    }
}

fn collect(nodes: &[OutputNode], parent: Option<u32>, ids: &mut Vec<(u32, Option<u32>)>) {
    for node in nodes {
        match node {
            OutputNode::Text(_) => {}
            OutputNode::Keep(id) => ids.push((*id, parent)),
            OutputNode::Element { id, nodes } => {
                ids.push((*id, parent));
                collect(nodes, Some(*id), ids);
            }
        }
    }
}

fn has_text_in(nodes: &[OutputNode], target_id: u32) -> bool {
    nodes.iter().any(|node| match node {
        OutputNode::Element { id, nodes } if *id == target_id => !flatten(nodes).trim().is_empty(),
        OutputNode::Element { nodes, .. } => has_text_in(nodes, target_id),
        _ => false,
    })
}

fn flatten(nodes: &[OutputNode]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            OutputNode::Text(text) => text.clone(),
            OutputNode::Keep(_) => String::new(),
            OutputNode::Element { nodes, .. } => flatten(nodes),
        })
        .collect()
}

#[tokio::test]
async fn failures_propagate_uncached() {
    let translator = translator();
    let limited = Fake::new(|_| Err(Error::RateLimited));
    for _ in 0..2 {
        let error = translator
            .translate(&limited, Job::new(japanese(), vec![text("Hello")]))
            .await
            .expect_err("request fails");
        assert!(matches!(error, Error::RateLimited), "{error:?}");
    }
    assert_eq!(limited.sizes(), [1, 1]);

    let short = Fake::new(|_| Ok(Vec::new()));
    let error = translator
        .translate(&short, Job::new(japanese(), vec![text("one"), text("two")]))
        .await
        .expect_err("request fails");
    assert!(matches!(error, Error::MalformedResponse), "{error:?}");
}

#[tokio::test]
async fn invalid_input_fails_before_request() {
    let fake = Fake::new(echo_upper);
    let translator = translator();
    let breton = Language::lookup("br").expect("Breton");
    let error = translator
        .translate(&fake, Job::new(breton, vec![text("Hello")]))
        .await
        .expect_err("unsupported");
    assert!(
        matches!(
            error,
            Error::Unsupported {
                service: "Fake",
                language: "br"
            }
        ),
        "{error:?}"
    );
    let duplicate = Unit::new(vec![
        Node::Keep {
            id: 1,
            text: String::new(),
        },
        Node::Keep {
            id: 1,
            text: String::new(),
        },
    ]);
    assert!(
        matches!(duplicate, Err(Error::DuplicateId(1))),
        "{duplicate:?}"
    );
    assert!(fake.sent().is_empty());
}
