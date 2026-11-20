use futures_util::StreamExt;
use rikka_llm::Effort;
use rikka_llm_accounts::ConnectedModel;
use rikka_test_llm::Scripted;
use rikka_thread::{Assistant, Desk, DeskEvent, Replies, Reply, Thread};
use rikka_tools::{Toolbox, facts};
use tokio::sync::mpsc;

pub(crate) const PNG: [u8; 72] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x7b, 0x40, 0xe8,
    0xdd, 0x00, 0x00, 0x00, 0x0f, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xf8, 0xcf, 0xc0, 0xc0,
    0xf0, 0x9f, 0x01, 0x00, 0x07, 0xff, 0x01, 0xff, 0xb8, 0x04, 0x35, 0xe0, 0x00, 0x00, 0x00, 0x00,
    0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

pub(crate) const LEAD: &str = "The person wrote this while you were working.";

pub(crate) fn project() -> (tempfile::TempDir, Assistant) {
    let folder = tempfile::tempdir().expect("temp dir should be created");
    std::fs::write(folder.path().join("a.txt"), "hello\n").expect("file should be written");
    let toolbox = Toolbox::builder(folder.path())
        .build()
        .expect("tools should be declared");
    let assistant = Assistant::builder(toolbox)
        .instruction("You are answering in a test.")
        .revision_heading("Your instructions changed.")
        .interjection_lead(LEAD)
        .build()
        .expect("prompts and tools should load");
    (folder, assistant)
}

pub(crate) fn tool(stem: &str) -> String {
    facts()
        .expect("shipped documents should parse")
        .get(&format!("{stem}_tool"))
        .expect("every tool should have a name")
        .to_owned()
}

pub(crate) fn connected_model(scripted: &Scripted) -> ConnectedModel {
    ConnectedModel::new(scripted.model(), None, Effort::default())
}

pub(crate) struct Watched {
    pub(crate) desk: Desk,
    events: mpsc::UnboundedReceiver<DeskEvent>,
}

impl Watched {
    pub(crate) fn new(assistant: Assistant) -> Watched {
        let (sender, events) = mpsc::unbounded_channel();
        let mut desk = Desk::new(Thread::new());
        desk.watch(move |event| {
            let _sent = sender.send(event);
        });
        let _previous = desk.equip(assistant);
        Watched { desk, events }
    }

    pub(crate) async fn drive(&mut self, replies: Replies) {
        self.respond(replies, |_, _| {}).await;
    }

    pub(crate) async fn respond(
        &mut self,
        replies: Replies,
        mut respond: impl FnMut(&mut Desk, &DeskEvent),
    ) -> Vec<Reply> {
        let mut replies = Box::pin(replies);
        let mut seen = Vec::new();
        loop {
            tokio::select! {
                reply = replies.next() => {
                    let Some(reply) = reply else {
                        return seen;
                    };
                    self.desk.apply(&reply);
                    seen.push(reply);
                }
                Some(event) = self.events.recv() => {
                    self.desk.heard(event.clone());
                    respond(&mut self.desk, &event);
                }
            }
        }
    }
}
