use paint_assistant::HistoryOrder;
use paint_document::Edit;
use rikka_tool::Reply;

use super::{Answer, Paint, PendingReply, reply};
use crate::action;

impl Paint {
    pub(super) fn travel(&mut self, index: usize, order: HistoryOrder, answer: Answer) {
        let history_view = match self.loaded(index) {
            Ok(tab) => tab.history_view().clone(),
            Err(refusal) => return reply(answer, Err(refusal)),
        };
        let (current, last) = (history_view.current, history_view.actions.len() - 1);
        let target = match order {
            HistoryOrder::List => {
                let saved = history_view
                    .saved
                    .map(|saved| format!(", saved at {saved}"))
                    .unwrap_or_default();
                let entries: String = history_view
                    .actions
                    .iter()
                    .enumerate()
                    .map(|(at, done)| {
                        let mark = if at == current { "*" } else { "" };
                        format!("\n{at}{mark} {}", action::name(done))
                    })
                    .collect();
                let text = format!(
                    "History of \"{}\", oldest first (* current{saved}):{entries}",
                    self.handle(index)
                );
                return reply(answer, Ok(Reply::new(text)));
            }
            HistoryOrder::Undo(steps) => current.saturating_sub(steps as usize),
            HistoryOrder::Redo(steps) => (current + steps as usize).min(last),
            HistoryOrder::Revisit(target) => {
                if target > last {
                    return reply(
                        answer,
                        Err(format!(
                            "there is no entry {target}; the history runs 0–{last}"
                        )),
                    );
                }
                target
            }
        };
        if target == current {
            let message = match order {
                HistoryOrder::Undo(_) => "there is nothing to undo".to_owned(),
                HistoryOrder::Redo(_) => "there is nothing to redo".to_owned(),
                _ => format!("the document is already at entry {current}"),
            };
            return reply(answer, Err(message));
        }
        if let Err(refusal) = self.editable(index) {
            return reply(answer, Err(refusal));
        }
        let tab = &mut self.tabs[index];
        let id = tab.id();
        match tab.errand(Edit::Revisit(target)) {
            Some(generation) => self.wait(id, PendingReply::Travel { generation }, answer),
            None => reply(answer, Err("the document has no worker".to_owned())),
        }
    }
}
