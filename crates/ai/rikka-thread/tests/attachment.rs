use futures_util::StreamExt;
use rikka_agent::Function;
use rikka_llm::{Content, Effort, Image, Media, ModelInfo, Tool, Value, json};
use rikka_llm_accounts::ConnectedModel;
use rikka_test_llm::Scripted;
use rikka_thread::{Ask, Assistant, Attachment, Desk, Reply, Thread};
use rikka_tools::Toolbox;

use crate::support::PNG;

#[test]
fn attachment_type_is_sniffed_from_content() {
    let at = tempfile::tempdir().expect("temp dir should be created");
    for (name, bytes) in [
        ("shot.PNG", PNG.as_slice()),
        ("notes.pdf", PNG.as_slice()),
        ("main.rs", b"fn main() {}".as_slice()),
        ("Makefile", b"all:\n\tcargo build\n".as_slice()),
        ("scan.txt", b"%PDF-1.7\n%\xc7\xec\x8f\xa2\n".as_slice()),
    ] {
        std::fs::write(at.path().join(name), bytes).expect("file should be written");
    }
    let read =
        |name: &str| Attachment::read(&at.path().join(name)).expect("attachment should read");

    let shot = read("shot.PNG");
    assert_eq!(shot.media(), Media::Png);
    assert_eq!(shot.name(), "shot.PNG");
    assert_eq!(shot.extent(), Some((2, 1)));
    assert!(matches!(shot.content(), Content::Image(_)));
    assert_eq!(read("notes.pdf").media(), Media::Png);
    let source = read("main.rs");
    assert_eq!(source.media(), Media::PlainText);
    assert!(matches!(source.content(), Content::Document(_)));
    assert_eq!(read("Makefile").media(), Media::PlainText);
    assert_eq!(read("scan.txt").media(), Media::Pdf);
}

#[test]
fn rejects_mismatched_attachment() {
    let at = tempfile::tempdir().expect("temp dir should be created");
    let huge = at.path().join("huge.png");
    std::fs::write(&huge, vec![0u8; 3 * 1024 * 1024 + 1]).expect("file should be written");
    std::fs::write(at.path().join("a.bin"), [0xff, 0xfe, 0x00]).expect("file should be written");
    std::fs::create_dir(at.path().join("shots")).expect("directory should be created");

    for (name, content) in [
        ("huge.png", "more than"),
        ("a.bin", "not a picture"),
        ("shots", "folder"),
    ] {
        let refused = Attachment::read(&at.path().join(name)).expect_err("it is refused");
        assert!(refused.to_string().contains(content), "{name}: {refused}");
    }
    assert!(Attachment::read(&at.path().join("nowhere.png")).is_err());
    assert!(Attachment::new("pasted.png", Media::Png, vec![0u8; 3 * 1024 * 1024 + 1]).is_err());

    let refused = Attachment::new("shot.png", Media::Png, *b"not a picture at all")
        .expect_err("no header can be read off it");
    assert!(refused.to_string().contains("not a picture"), "{refused}");
    let lied =
        Attachment::new("shot.jpg", Media::Jpeg, PNG).expect_err("the header says something else");
    assert!(lied.to_string().contains("image/png"), "{lied}");
}

fn looking(project: &std::path::Path) -> Assistant {
    let tool = Tool::new("Look")
        .description("Looks at the screen")
        .build()
        .expect("tool should be valid");
    let looked = Function::new(tool, |_: Value| async {
        Ok::<_, String>(rikka_agent::Reply::new("the screen").image(Image::png(PNG)))
    });
    let toolbox = Toolbox::builder(project)
        .tools([looked])
        .build()
        .expect("tools should be declared");
    Assistant::builder(toolbox)
        .instruction("You are answering in a test.")
        .revision_heading("Your instructions changed.")
        .build()
        .expect("prompts and tools should load")
}

#[tokio::test]
async fn model_without_vision_is_told_of_pictures_instead_of_shown_them() {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    let shot = Attachment::new("shot.png", Media::Png, PNG).expect("picture should attach");

    for (vision, shown) in [(true, true), (false, false)] {
        let scripted = Scripted::new();
        scripted.asks("Look", json!({})).says("Seen.");
        let info = ModelInfo::new("scripted", "Scripted", 100_000, 1_000).with_vision(vision);
        let mut desk = Desk::new(Thread::new());
        let _unequipped = desk.equip(looking(folder.path()));
        let replies: Vec<Reply> = desk
            .ask(
                ConnectedModel::new(scripted.model(), Some(info), Effort::default()),
                Ask::new("what is this?").with_attachments(vec![shot.clone()]),
            )
            .expect("agent should build")
            .collect()
            .await;
        assert!(matches!(replies.last(), Some(Reply::Done)), "{replies:?}");

        let requests = scripted.requests();
        let last = requests.last().expect("the model should be asked").body();
        let sent = last["messages"].to_string();
        let unseen = sent.matches("cannot see pictures").count();
        assert_eq!(sent.contains("\"image\""), shown, "vision {vision}: {sent}");
        assert_eq!(unseen, if shown { 0 } else { 2 }, "vision {vision}: {sent}");
        assert!(sent.contains("what is this?") && sent.contains("the screen"));
    }
}
