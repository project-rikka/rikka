use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver};

use rikka_notifications::{
    CloseReason, Emphasis, Link, Notifier, Posted, Progress, Style, Urgency,
};
use rikka_test_bus::{PrivateBus, TIMEOUT};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::Value;

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

#[test]
fn server_follows_notification_spec() {
    let Some(_bus) = PrivateBus::start() else {
        return;
    };

    let (wake, woken) = mpsc::channel();
    let notifier = Notifier::start(move || {
        wake.send(()).expect("test waits for wakeups");
    })
    .expect("bus name should be free on a private bus");

    let client = Connection::session().expect("second bus connection should open");
    let proxy = Proxy::new(&client, NAME, PATH, NAME).expect("server should exist");

    let capabilities: Vec<String> = proxy
        .call("GetCapabilities", &())
        .expect("capabilities should be listed");
    assert!(capabilities.contains(&"body-markup".to_owned()));
    assert!(
        capabilities.contains(&"persistence".to_owned()),
        "one nobody got to waits in the desktop's notification centre"
    );
    assert!(
        !capabilities.contains(&"sound".to_owned()),
        "nothing is claimed that is not done"
    );

    let (_, _, _, specification): (String, String, String, String) = proxy
        .call("GetServerInformation", &())
        .expect("server should describe itself");
    assert_eq!(specification, "1.2");

    let mut closed = proxy
        .receive_signal("NotificationClosed")
        .expect("signal match rule should register");

    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
    hints.insert("urgency", Value::U8(2));
    hints.insert("desktop-entry", Value::from("org.rikka.Files"));
    hints.insert("value", Value::I32(40));
    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Spotify",
                0u32,
                "spotify",
                "The LP",
                "<b>Large Professor</b> - The LP",
                vec!["default", "Open"],
                hints,
                -1i32,
            ),
        )
        .expect("notification should be accepted");
    assert_ne!(id, 0, "zero names no notification");

    let posted = next_posted(&notifier, &woken);
    let [Posted::Shown(notification)] = posted.as_slice() else {
        panic!("one notification arrived, and it is the one that was sent: {posted:?}");
    };
    assert_eq!(notification.id, id);
    assert_eq!(notification.app_name, "Spotify");
    assert_eq!(notification.body.text, "Large Professor - The LP");
    assert_eq!(notification.body.emphasis.len(), 1, "the markup was read");
    assert_eq!(notification.urgency, Urgency::Critical);
    assert_eq!(
        notification.desktop_entry.as_deref(),
        Some("org.rikka.Files")
    );
    assert!(notification.default_action().is_some());
    assert_eq!(notification.progress, Some(Progress::Percent(40)));

    let mut signals = proxy
        .receive_all_signals()
        .expect("signal match rules should register");
    notifier.activate(id, "default", Some("rikka-token"));
    notifier.activate(id, "default", None);
    let signals_seen: Vec<(String, u32, String)> = signals
        .by_ref()
        .take(3)
        .map(|signal| {
            let member = signal
                .header()
                .member()
                .map(|member| member.to_string())
                .unwrap_or_default();
            let (id, body): (u32, String) = signal
                .body()
                .deserialize()
                .expect("signal body should hold id and text");
            (member, id, body)
        })
        .collect();
    assert_eq!(
        signals_seen,
        [
            ("ActivationToken".to_owned(), id, "rikka-token".to_owned()),
            ("ActionInvoked".to_owned(), id, "default".to_owned()),
            ("ActionInvoked".to_owned(), id, "default".to_owned()),
        ],
        "the token comes right before the action it belongs to, and only when there is one"
    );

    notifier.close(id, CloseReason::Dismissed);
    let closed_signal = closed.next().expect("closed signal should arrive");
    let (gone, reason): (u32, u32) = closed_signal
        .body()
        .deserialize()
        .expect("signal body should hold id and reason");
    assert_eq!((gone, reason), (id, 2));

    let () = proxy
        .call("CloseNotification", &id)
        .expect("unknown id should get a reply");
    notifier.close(id, CloseReason::Expired);

    let next: u32 = proxy
        .call(
            "Notify",
            &(
                "Files",
                0u32,
                "",
                "Copy finished",
                "",
                Vec::<&str>::new(),
                HashMap::<&str, Value<'_>>::new(),
                -1i32,
            ),
        )
        .expect("second notification should be accepted");
    notifier.close(next, CloseReason::Expired);

    let closed_signal = closed.next().expect("second error should be reported");
    let (gone, reason): (u32, u32) = closed_signal
        .body()
        .deserialize()
        .expect("signal body should hold id and reason");
    assert_eq!((gone, reason), (next, 1));

    drop(notifier.drain());
    for (markup, text) in [
        ("<b>one <i>two</i></b>", "one two"),
        ("<u>never closed", "never closed"),
        ("one<br>two<br/>three", "one\ntwo\nthree"),
        ("a &amp; b &lt; c &#65; &#x42;", "a & b < c A B"),
        ("2 < 3 && 4 > 1", "2 < 3 && 4 > 1"),
        (
            "<span foreground='red'>red</span> and <tt>fixed</tt>",
            "red and fixed",
        ),
        (
            "<img src=\"cover.png\" alt=\"Album cover\"/> The LP",
            "Album cover The LP",
        ),
        ("one&#x202E;two&#0;three\u{202e}four", "onetwothreefour"),
        ("plain</b> text", "plain text"),
        (
            "see <a href=\"https://rikka.os/x?a=1&amp;b=2\">the notes</a>",
            "see the notes",
        ),
    ] {
        let _: u32 = proxy
            .call(
                "Notify",
                &(
                    "Files",
                    0u32,
                    "",
                    "Markup",
                    markup,
                    Vec::<&str>::new(),
                    HashMap::<&str, Value<'_>>::new(),
                    -1i32,
                ),
            )
            .expect("notification should be accepted");
        let posted = next_posted(&notifier, &woken);
        let [Posted::Shown(shown_notification)] = posted.as_slice() else {
            panic!("one notification arrived: {posted:?}");
        };
        assert_eq!(shown_notification.body.text, text, "{markup}");
        match markup {
            "<b>one <i>two</i></b>" => assert_eq!(
                shown_notification.body.emphasis,
                [
                    Emphasis {
                        range: 4..7,
                        style: Style::Italic,
                    },
                    Emphasis {
                        range: 0..7,
                        style: Style::Bold,
                    },
                ]
            ),
            "<u>never closed" => assert_eq!(shown_notification.body.emphasis[0].range, 0..12),
            "<span foreground='red'>red</span> and <tt>fixed</tt>" => {
                assert!(shown_notification.body.emphasis.is_empty());
            }
            _ if markup.starts_with("see") => assert_eq!(
                shown_notification.body.links,
                [Link {
                    range: 4..13,
                    href: "https://rikka.os/x?a=1&b=2".to_owned(),
                }]
            ),
            _ => {}
        }
    }
}

fn next_posted(notifier: &Notifier, woken: &Receiver<()>) -> Vec<Posted> {
    loop {
        woken
            .recv_timeout(TIMEOUT)
            .expect("notifier wakes the desktop");
        let posted = notifier.drain();
        if !posted.is_empty() {
            return posted;
        }
    }
}
