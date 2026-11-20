mod change;
mod compose;
mod document;
mod files;
mod history;
mod layers;
mod measure;
mod objects;
mod pixels;
mod view;

use paint_assistant::{Order, Request};
use paint_document::{Action, Region, Span};
use rikka_app::prelude::*;
use rikka_tool::Reply;
use tokio::sync::oneshot;

use super::Paint;
use crate::assistant;
use crate::tab::{Phase, Tab, TabEvent};
use layers::LayerTree;

pub(crate) type ServeResult = Result<Reply, String>;
pub(crate) type Answer = oneshot::Sender<ServeResult>;

pub(crate) struct Waiting {
    tab: u64,
    on: PendingReply,
    answer: Answer,
}

enum PendingReply {
    Change {
        generation: u64,
        name: String,
        lists: bool,
    },
    Travel {
        generation: u64,
    },
    Save,
    Open,
}

fn reply(answer: Answer, result: ServeResult) {
    let _ = answer.send(result);
}

impl Paint {
    pub(super) fn offer(&mut self) {
        self.seat = assistant::seat(self.sender.clone(), self.seat_title());
    }

    pub(crate) fn focused(&mut self) {
        if let Some(seat) = &self.seat {
            seat.mark_active();
        }
        skip_view();
    }

    pub(crate) fn open_assistant(&mut self) {
        assistant::open_assistant();
        skip_view();
    }

    fn seat_title(&self) -> String {
        match self.tabs.is_empty() {
            true => "No document".to_owned(),
            false => self
                .handles()
                .into_iter()
                .collect::<Vec<String>>()
                .join(", "),
        }
    }

    pub(super) fn retitle(&self) {
        if let Some(seat) = &self.seat {
            seat.set_title(self.seat_title());
        }
    }

    pub(crate) fn serve(&mut self, order: Order, answer: Answer) {
        self.prune();
        let Order { document, request } = order;
        if let Request::Open(path) = request {
            return self.open_for(path, answer);
        }
        if let Request::New(made) = request {
            return self.create_for(made, answer);
        }
        let index = match self.tab_called(document.as_deref()) {
            Ok(index) => index,
            Err(refusal) => return reply(answer, Err(refusal)),
        };
        match request {
            Request::Document => reply(answer, self.describe(index)),
            Request::View(viewing) => self.view(index, viewing, answer),
            Request::Measure(measuring) => self.measure(index, measuring, answer),
            Request::History(travel) => self.travel(index, travel, answer),
            Request::Save(saving) => self.save_for(index, saving, answer),
            Request::Change { name, change } => self.change(index, name, change, answer),
            Request::Open(_) | Request::New(_) => {}
        }
    }

    pub(super) fn handle_tab_event(&mut self, tab: u64, event: &TabEvent) {
        match event {
            TabEvent::OrderDone {
                generation,
                refusal,
                notes,
            } => {
                self.settle(tab, |on| match on {
                    PendingReply::Change {
                        generation: awaited,
                        ..
                    }
                    | PendingReply::Travel {
                        generation: awaited,
                    } => awaited == generation,
                    PendingReply::Save | PendingReply::Open => false,
                })
                .into_iter()
                .for_each(|(on, answer)| {
                    let result = match refusal {
                        Some(refusal) => Err(format!("Nothing was changed: {refusal}")),
                        None => self.landed(tab, &on, notes),
                    };
                    reply(answer, result);
                });
            }
            TabEvent::Saved { path, .. } => {
                let message = format!("Saved to {}.", path.display());
                self.answer_all(
                    tab,
                    |on| matches!(on, PendingReply::Save),
                    Ok(Reply::new(message)),
                );
            }
            TabEvent::Copied { path } => {
                let message = format!(
                    "Saved a flattened copy to {}; the layered document stays bound to its own file.",
                    path.display()
                );
                self.answer_all(
                    tab,
                    |on| matches!(on, PendingReply::Save),
                    Ok(Reply::new(message)),
                );
            }
            TabEvent::Unsaved => {
                let reason = self
                    .tabs
                    .iter()
                    .find(|candidate| candidate.id() == tab)
                    .and_then(Tab::error_message)
                    .map_or_else(String::new, ToString::to_string);
                self.answer_all(
                    tab,
                    |on| matches!(on, PendingReply::Save),
                    Err(format!("The document was not saved: {reason}")),
                );
            }
            TabEvent::Opened(_) => {
                if let Some(index) = self.tabs.iter().position(|candidate| candidate.id() == tab) {
                    let described = self.describe(index);
                    self.answer_all(tab, |on| matches!(on, PendingReply::Open), described);
                }
            }
            TabEvent::Nothing | TabEvent::Sampled { .. } => {}
        }
        let failed = self
            .tabs
            .iter()
            .find(|candidate| candidate.id() == tab)
            .and_then(|tab_ref| match tab_ref.phase() {
                Phase::Failed(reason) => Some(reason.to_string()),
                Phase::Opening | Phase::Open(_) => None,
            });
        if let Some(reason) = failed {
            self.answer_all(
                tab,
                |on| matches!(on, PendingReply::Open),
                Err(format!("The file could not be opened: {reason}")),
            );
        }
    }

    fn settle(
        &mut self,
        tab: u64,
        matches: impl Fn(&PendingReply) -> bool,
    ) -> Vec<(PendingReply, Answer)> {
        let (ready, pending): (Vec<Waiting>, Vec<Waiting>) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|waiting| waiting.tab == tab && matches(&waiting.on));
        self.waiting = pending;
        ready
            .into_iter()
            .map(|waiting| (waiting.on, waiting.answer))
            .collect()
    }

    fn answer_all(
        &mut self,
        tab: u64,
        matches: impl Fn(&PendingReply) -> bool,
        result: ServeResult,
    ) {
        for (_, answer) in self.settle(tab, matches) {
            reply(answer, result.clone());
        }
    }

    fn wait(&mut self, tab: u64, on: PendingReply, answer: Answer) {
        self.waiting.push(Waiting { tab, on, answer });
    }

    fn prune(&mut self) {
        let (pending, orphaned): (Vec<Waiting>, Vec<Waiting>) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|waiting| self.tabs.iter().any(|tab| tab.id() == waiting.tab));
        self.waiting = pending;
        for waiting in orphaned {
            reply(
                waiting.answer,
                Err(
                    "the document was closed or moved to another window before the change landed"
                        .to_owned(),
                ),
            );
        }
    }

    fn landed(&self, tab: u64, on: &PendingReply, notes: &[String]) -> ServeResult {
        let Some(tab_ref) = self.tabs.iter().find(|candidate| candidate.id() == tab) else {
            return Err("the document was closed".to_owned());
        };
        let history_view = tab_ref.history_view();
        let current = history_view.current;
        let last = history_view.actions.len().saturating_sub(1);
        let size = tab_ref
            .facts()
            .map(|facts| format!("{}×{}", facts.width, facts.height))
            .unwrap_or_default();
        let selection = selection_said(&tab_ref.selection().region());
        match on {
            PendingReply::Change { name, lists, .. } => {
                if history_view.actions.get(current) != Some(&Action::Batch(name.clone())) {
                    return Ok(Reply::new(format!(
                        "Nothing changed: \"{name}\" left the document as it was."
                    )));
                }
                let mut parts = vec![format!(
                    "Done: \"{name}\" is history entry {current} of {last}. The canvas is {size}; selection {selection}."
                )];
                parts.extend(notes.iter().cloned());
                if *lists {
                    let active = tab_ref
                        .active_layer()
                        .map(|active_layer| {
                            format!(
                                "Chosen: {} \"{}\"",
                                active_layer.id.index(),
                                layers::shown_name(active_layer)
                            )
                        })
                        .unwrap_or_default();
                    parts.push(format!(
                        "Layers now, top first:\n{}{active}",
                        LayerTree(tab_ref.layers())
                    ));
                }
                Ok(Reply::new(parts.join("\n")))
            }
            PendingReply::Travel { .. } => {
                let label = history_view
                    .actions
                    .get(current)
                    .map(|action| crate::action::name(action).to_string())
                    .unwrap_or_default();
                Ok(Reply::new(format!(
                    "Now at history entry {current} of {last}, \"{label}\". The canvas is {size}; selection {selection}."
                )))
            }
            PendingReply::Save | PendingReply::Open => Ok(Reply::new("Done.")),
        }
    }

    fn handles(&self) -> Vec<String> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(at, tab)| {
                let name = tab.name();
                let before = self.tabs[..at]
                    .iter()
                    .filter(|earlier| earlier.name() == name)
                    .count();
                match before {
                    0 => name,
                    more => format!("{name} ({})", more + 1),
                }
            })
            .collect()
    }

    fn tab_called(&self, name: Option<&str>) -> Result<usize, String> {
        if self.tabs.is_empty() {
            return Err(format!(
                "no document is open in this window; open one with {}",
                exposed_tool("Open")
            ));
        }
        let Some(name) = name else {
            return Ok(self.active.min(self.tabs.len() - 1));
        };
        let handles = self.handles();
        handles
            .iter()
            .position(|handle| handle == name)
            .ok_or_else(|| {
                format!(
                    "there is no document \"{name}\"; the documents are {}",
                    handles
                        .iter()
                        .map(|handle| format!("\"{handle}\""))
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            })
    }

    fn handle(&self, index: usize) -> String {
        self.handles().swap_remove(index)
    }

    fn loaded(&self, index: usize) -> Result<&Tab, String> {
        let tab = &self.tabs[index];
        match tab.phase() {
            Phase::Open(_) => Ok(tab),
            Phase::Opening => Err(format!(
                "\"{}\" is still opening; try again in a moment",
                self.handle(index)
            )),
            Phase::Failed(reason) => Err(format!(
                "\"{}\" could not be opened: {reason}",
                self.handle(index)
            )),
        }
    }

    fn editable(&self, index: usize) -> Result<&Tab, String> {
        let tab = self.loaded(index)?;
        let name = self.handle(index);
        if self.unsaved_prompt.is_some() {
            return Err(
                "Paint is asking the person about unsaved changes; wait until they answer"
                    .to_owned(),
            );
        }
        if let Some(session) = tab.session() {
            return Err(format!(
                "\"{name}\" has the {} dialog open; the person must apply or cancel it first",
                session.dialog().title()
            ));
        }
        if tab.is_reshaping() {
            return Err(format!(
                "a crop or resize of \"{name}\" is still being applied; call again in a moment"
            ));
        }
        Ok(tab)
    }
}

fn exposed_tool(tool: &str) -> String {
    rikka_assist::exposed(paint_assistant::KIT.name(), tool)
}

fn selection_said(region: &Region) -> String {
    let Some(area) = region.bounds() else {
        return "none".to_owned();
    };
    let spans = || region.rows().flat_map(|(_, spans)| spans.iter());
    let covered: f64 = spans()
        .map(|span| f64::from(span.end - span.start) * f64::from(span.coverage))
        .sum::<f64>()
        / f64::from(Span::FULL);
    let soft = spans().any(|span| span.coverage < Span::FULL);
    let shape = match (region.rectangular(), soft) {
        (Some(_), _) => ", a rectangle",
        (None, true) => ", soft edges",
        (None, false) => "",
    };
    format!(
        "x {}–{}, y {}–{} ({}×{}, {:.0} px{shape})",
        area.left,
        area.left + area.width,
        area.top,
        area.top + area.height,
        area.width,
        area.height,
        covered,
    )
}
