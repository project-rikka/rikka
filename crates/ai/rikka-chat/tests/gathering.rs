use rikka_chat::{Labels, gather, kept_attachments, prompt_offers, resource_offers};
use rikka_llm::Media;
use rikka_mcp::{Catalog, Hub, Place, Server, Stdio};
use rikka_test_mcp::echo_server;
use rikka_thread::{Ask, Attachment};
use rikka_ui::Label;
use tempfile::TempDir;

struct Plain;

impl Labels for Plain {
    fn placeholder() -> Label {
        Label::from("")
    }
    fn answering() -> Label {
        Label::from("")
    }
    fn folding() -> Label {
        Label::from("")
    }
    fn folded() -> Label {
        Label::from("")
    }
    fn noticed() -> Label {
        Label::from("")
    }
    fn main_thread() -> Label {
        Label::from("")
    }
    fn more_lines(_count: usize) -> Label {
        Label::from("")
    }
    fn pick_model() -> Label {
        Label::from("")
    }
    fn reasoning() -> Label {
        Label::from("")
    }
    fn write_answer() -> Label {
        Label::from("")
    }
    fn stop_asking() -> Label {
        Label::from("")
    }
    fn back() -> Label {
        Label::from("")
    }
    fn next() -> Label {
        Label::from("")
    }
    fn send_answers() -> Label {
        Label::from("")
    }
    fn choose_files() -> Label {
        Label::from("")
    }
    fn attach() -> Label {
        Label::from("")
    }
    fn unreadable() -> Label {
        Label::from("")
    }
    fn refused(reason: &str) -> Label {
        Label::from(reason)
    }
    fn refused_more(count: usize) -> Label {
        Label::from(format!("{count} more"))
    }
    fn dropped() -> Label {
        Label::from("")
    }
    fn pasted() -> Label {
        Label::from("")
    }
    fn compact_hint() -> Label {
        Label::from("")
    }
    fn compact_about() -> Label {
        Label::from("")
    }
}

async fn connected() -> (TempDir, Hub) {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let hub = Hub::builder()
        .place(Place::new("assistant", folder.path().join("mcp.json")))
        .build();
    hub.save_catalog(
        "assistant",
        Catalog::from_servers([Server::new("echo", Stdio::new(echo_server(folder.path())))]),
    )
    .await
    .expect("catalog should be written");
    (folder, hub)
}

fn names(ask: &Ask) -> Vec<&str> {
    ask.attachments()
        .iter()
        .map(|attachment| attachment.name())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn draft_offers_prompts_and_resources() {
    let (_folder, hub) = connected().await;

    let prompts = prompt_offers(&hub, "ec");
    let spelt: Vec<(&str, bool)> = prompts
        .iter()
        .map(|offer| (offer.spelt(), offer.runs()))
        .collect();
    assert_eq!(spelt, [("echo:greet", false)]);
    assert_eq!(prompts[0].insertion(), "/echo:greet ");

    let resources = resource_offers(&hub, "echo:");
    let insertions: Vec<&str> = resources.iter().map(|offer| offer.insertion()).collect();
    assert_eq!(insertions, ["@echo:note://hello ", "@echo:note://"]);
    assert!(resource_offers(&hub, "src/").is_empty());
}

#[test]
fn mentioned_paths_are_not_kept_attachments() {
    let attachment = |name: &str| {
        Attachment::new(name, Media::PlainText, b"text".to_vec()).expect("attachment should fit")
    };
    let said = "use @assets/ with @\"docs/a b.md\"";
    let attachments = [
        attachment("assets/"),
        attachment("docs/a b.md"),
        attachment("pasted.txt"),
    ];
    let kept = kept_attachments(said, said, &attachments);
    let kept: Vec<&str> = kept.iter().map(Attachment::name).collect();
    assert_eq!(kept, ["pasted.txt"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn prompt_and_resource_mentions_gather_into_ask() {
    let (_folder, hub) = connected().await;

    let said = "/echo:greet Ada 'very warmly'";
    let prompted = gather::<Plain>(&hub, Ask::new(said))
        .await
        .expect("prompt should be expanded");
    let ask = prompted
        .ask
        .clone()
        .with_attachments(prompted.attached.attachments.clone());
    assert_eq!(ask.text(), said);
    assert_eq!(ask.sent_text(), "Greet Ada, very warmly");
    assert_eq!(names(&ask), ["/echo:greet/note://hello"]);
    assert!(prompted.mentions.is_empty());
    assert!(kept_attachments(ask.text(), ask.sent_text(), ask.attachments()).is_empty());

    let said = "compare @echo:note://hello with @notes.md";
    let mentioned = gather::<Plain>(&hub, Ask::new(said))
        .await
        .expect("resources should be read");
    assert_eq!(mentioned.ask.sent_text(), said);
    assert_eq!(mentioned.mentions, ["notes.md"]);
    assert_eq!(mentioned.attached.refused, None);
    let read = &mentioned.attached.attachments;
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].name(), "echo:note://hello");
    assert_eq!(read[0].data().as_ref(), b"hello there");

    assert!(
        gather::<Plain>(&hub, Ask::new("/echo:greet"))
            .await
            .is_err()
    );
}
