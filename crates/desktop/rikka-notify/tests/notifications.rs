use futures_util::StreamExt;
use rikka_notifications::{CloseReason as Reason, Image, Notifier, Posted, Urgency as Level};
use rikka_notify::{CloseReason, Event, Notification, Notifications, Picture, Urgency};
use rikka_test_bus::{PrivateBus, TIMEOUT};
use tokio::sync::mpsc::{self, UnboundedReceiver};

async fn posted(notifier: &Notifier, woken: &mut UnboundedReceiver<()>) -> Vec<Posted> {
    tokio::time::timeout(TIMEOUT, async {
        loop {
            woken.recv().await.expect("notifier is running");
            let posted = notifier.drain();
            if !posted.is_empty() {
                return posted;
            }
        }
    })
    .await
    .expect("notification should reach the server in time")
}

async fn next(events: &mut rikka_notify::Events) -> Event {
    tokio::time::timeout(TIMEOUT, events.next())
        .await
        .expect("event should arrive in time")
        .expect("event stream should stay open")
}

#[tokio::test]
async fn notification_round_trips_actions() {
    let Some(_bus) = PrivateBus::start() else {
        return;
    };
    let client = Notifications::new();
    assert!(
        matches!(
            client.notify(&Notification::new("Nobody listens")).await,
            Err(rikka_notify::Error::NoServer)
        ),
        "a session without a notification server says so"
    );

    let (wake, mut woken) = mpsc::unbounded_channel();
    let notifier = Notifier::start(move || {
        wake.send(()).expect("test waits for wakeups");
    })
    .expect("the name is free on a bus of our own");
    let mut events = client.events();
    let picture = Picture::rgba(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 128]).expect("two pixels");
    let notification = Notification::new("Message from example.org")
        .body("Lunch at noon?")
        .app_name("Browser")
        .desktop_entry("rikka-browser")
        .urgency(Urgency::Critical)
        .default_action("Show")
        .action("mute", "Mute example.org")
        .icon(picture);
    let id = client
        .notify(&notification)
        .await
        .expect("the desktop takes it");

    let shown = posted(&notifier, &mut woken).await;
    let [Posted::Shown(shown)] = shown.as_slice() else {
        panic!("one notification arrived: {shown:?}");
    };
    assert_eq!(shown.id, id.get());
    assert_eq!(shown.app_name, "Browser");
    assert_eq!(shown.summary, "Message from example.org");
    assert_eq!(shown.body.text, "Lunch at noon?");
    assert_eq!(shown.urgency, Level::Critical);
    assert_eq!(shown.desktop_entry.as_deref(), Some("rikka-browser"));
    assert!(shown.default_action().is_some());
    let Some(Image::Pixels(pixels)) = &shown.image else {
        panic!("the picture travelled as pixels: {:?}", shown.image);
    };
    assert_eq!((pixels.width, pixels.height), (2, 1));

    let stranger = notifier.post(rikka_notifications::Notification::new("Files", "Copied"));
    notifier.activate(stranger, "default", Some("someone-else"));
    notifier.activate(id.get(), "mute", Some("rikka-token"));
    assert_eq!(
        next(&mut events).await,
        Event::ActivationToken {
            id,
            token: "rikka-token".to_owned(),
        },
        "the token to show the window with comes before the action"
    );
    assert_eq!(
        next(&mut events).await,
        Event::ActionInvoked {
            id,
            key: "mute".to_owned(),
        },
        "only what this application posted is heard"
    );

    notifier.close(id.get(), Reason::Dismissed);
    assert_eq!(
        next(&mut events).await,
        Event::Closed {
            id,
            reason: CloseReason::Dismissed,
        }
    );

    let second = client
        .notify(&Notification::new("Download finished").replaces(id))
        .await
        .expect("a second is taken");
    let _ = posted(&notifier, &mut woken).await;
    client
        .close(second)
        .await
        .expect("the desktop is asked to close it");
    let withdrawn = posted(&notifier, &mut woken).await;
    assert!(
        matches!(withdrawn.as_slice(), [Posted::Withdrawn(gone)] if *gone == second.get()),
        "the notification was withdrawn: {withdrawn:?}"
    );
}
