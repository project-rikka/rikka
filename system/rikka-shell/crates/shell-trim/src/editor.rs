use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use rikka_seam::Request;
use rikka_session::following;
use rikka_session::replay::Saved;
use rikka_ui::{State, cancel, skip_view, spawn_blocking, stream, stream_keyed};
use rikka_video::Keyframes;
use shell_kit::{CLOSE, Spot};

use crate::cut::Cut;
use crate::error::{Error, Result};
use crate::opening::Opening;
use crate::preview::Preview;
use crate::replays::{self, Strings};
use crate::service::{ARRIVED, CAPABILITY};
use crate::shared::Shared;
use crate::writing;
const WATCHING: &str = "shell.trim.watching";

const NUDGE: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edge {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Back,
    On,
}

#[derive(State)]
pub(crate) struct Editor {
    shared: Shared,
    pending_open: Option<Opening>,
    opening: u64,
    preview: Option<Preview>,
    keyframes: Option<Rc<Keyframes>>,
    cut: Option<Cut>,
    trouble: Option<Arc<Error>>,
    working: bool,
}

impl Editor {
    #[must_use]
    pub(crate) fn new(shared: Shared) -> Editor {
        Editor {
            shared,
            pending_open: None,
            opening: 0,
            preview: None,
            keyframes: None,
            cut: None,
            trouble: None,
            working: false,
        }
    }

    pub(crate) fn start(&mut self) {
        let arrived = following::changes(&self.shared.seam, CAPABILITY, ARRIVED);
        stream(arrived, |editor: &mut Editor, ()| editor.arrived());
        self.arrived();
    }

    #[must_use]
    pub(crate) fn spot(&self) -> &Spot {
        &self.shared.spot
    }

    #[must_use]
    pub(crate) fn preview(&self) -> Option<&Preview> {
        self.preview.as_ref()
    }

    #[must_use]
    pub(crate) fn cut(&self) -> Option<Cut> {
        self.cut
    }

    #[must_use]
    pub(super) fn keyframes(&self) -> Option<Rc<Keyframes>> {
        self.keyframes.clone()
    }

    #[must_use]
    pub(crate) fn trouble(&self) -> Option<&Error> {
        self.trouble.as_deref()
    }

    #[must_use]
    pub(crate) fn is_ready(&self) -> bool {
        self.cut.is_some() && self.keyframes.is_some() && self.trouble.is_none()
    }

    #[must_use]
    pub(crate) fn is_working(&self) -> bool {
        self.working
    }

    #[must_use]
    pub(super) fn is_draft(&self) -> bool {
        self.pending_open
            .as_ref()
            .is_some_and(|pending_open| pending_open.draft)
    }

    #[must_use]
    pub(crate) fn is_unsaved(&self) -> bool {
        let draft = self.is_draft();
        self.cut.is_some_and(|cut| draft || !cut.is_whole())
    }

    fn arrived(&mut self) {
        if !self.shared.waiting.is_waiting() {
            return skip_view();
        }
        let Some(opening) = self.shared.waiting.take() else {
            return skip_view();
        };
        self.take(opening);
    }

    fn take(&mut self, opening: Opening) {
        if self
            .pending_open
            .as_ref()
            .is_some_and(|pending_open| pending_open.file != opening.file)
        {
            self.release();
        }
        self.forget();
        let preview = Preview::open(&opening.file);
        stream_keyed(WATCHING, preview.changes(), |editor: &mut Editor, ()| {
            editor.moved_on();
        });
        self.preview = Some(preview);
        self.pending_open = Some(opening);
        self.scan();
    }

    fn scan(&self) {
        let Some(pending_open) = self.pending_open.as_ref() else {
            return;
        };
        let file = pending_open.file.clone();
        let mark = self.opening;
        spawn_blocking(
            move || Keyframes::scan(&file).map_err(Error::from),
            move |editor: &mut Editor, scanned: Result<Keyframes>| {
                editor.scanned(mark, scanned.map_err(Arc::new));
            },
        );
    }

    fn scanned(&mut self, mark: u64, scanned: Result<Keyframes, Arc<Error>>) {
        if mark != self.opening {
            return skip_view();
        }
        match scanned {
            Ok(keyframes) => self.keyframes = Some(Rc::new(keyframes)),
            Err(error) => {
                tracing::warn!("cannot trim recording: {error}");
                self.trouble = Some(error);
            }
        }
    }

    fn moved_on(&mut self) {
        let cut = self.cut;
        let Some(preview) = self.preview.as_mut() else {
            return skip_view();
        };
        if let Some(cut) = cut
            && preview.is_playing()
            && preview.position() >= cut.end()
        {
            preview.pause();
        }
        let trouble = preview.trouble();
        let length = preview.length();
        let preview_open = preview.is_open();

        if let Some(trouble) = trouble {
            self.trouble
                .get_or_insert_with(|| Arc::new(Error::Unplayable(trouble)));
            return cancel(WATCHING);
        }
        if self.cut.is_none() {
            match length {
                Some(length) => self.cut = Some(Cut::whole(length)),
                None if preview_open => {
                    self.trouble = Some(Arc::new(Error::Unmeasured));
                    cancel(WATCHING);
                }
                None => {}
            }
        }
    }

    pub(super) fn play_pause(&mut self) {
        let Some(cut) = self.cut else {
            return skip_view();
        };
        let Some(preview) = self.preview.as_mut() else {
            return skip_view();
        };
        if preview.is_playing() {
            preview.pause();
            return;
        }
        if !cut.holds(preview.position()) {
            preview.moved_to(cut.start());
        }
        preview.play();
    }

    pub(super) fn scrubbed(&mut self, at: Duration) {
        let Some(cut) = self.cut else {
            return skip_view();
        };
        let Some(preview) = self.preview.as_mut() else {
            return skip_view();
        };
        preview.moved_to(at.min(cut.length()));
    }

    pub(super) fn nudged(&mut self, step: Step) {
        let Some(at) = self.preview.as_ref().map(Preview::position) else {
            return skip_view();
        };
        let moved = match step {
            Step::Back => at.saturating_sub(NUDGE),
            Step::On => at + NUDGE,
        };
        self.scrubbed(moved);
    }

    pub(crate) fn moved(&mut self, edge: Edge, at: Duration) {
        let Some(mut cut) = self.cut else {
            return skip_view();
        };
        let changed = match edge {
            Edge::Start => cut.started_at(at),
            Edge::End => cut.ended_at(at),
        };
        if !changed {
            return skip_view();
        }
        self.cut = Some(cut);
        let moved = match edge {
            Edge::Start => cut.start(),
            Edge::End => cut.end(),
        };
        if let Some(preview) = self.preview.as_mut() {
            preview.moved_to(moved);
        }
    }

    fn settle(&mut self) -> bool {
        let (Some(mut cut), Some(keyframes)) = (self.cut, self.keyframes.clone()) else {
            return false;
        };
        if !cut.settled(&keyframes) {
            return false;
        }
        self.cut = Some(cut);
        true
    }

    pub(crate) fn settled(&mut self) {
        if !self.settle() {
            return skip_view();
        }
        let Some(start) = self.cut.map(Cut::start) else {
            return skip_view();
        };
        if let Some(preview) = self.preview.as_mut() {
            preview.moved_to(start);
        }
    }

    pub(crate) fn save(&mut self) {
        let settled = self.settle();
        let nothing = move || {
            if !settled {
                skip_view();
            }
        };
        if !self.is_unsaved() {
            return nothing();
        }
        let (Some(pending_open), Some(cut)) = (self.pending_open.as_ref(), self.cut) else {
            return nothing();
        };
        if self.working {
            return nothing();
        }
        let file = pending_open.file.clone();
        let draft = pending_open.draft;
        let span = cut.span();
        let mark = self.opening;
        self.working = true;
        self.trouble = None;
        if let Some(preview) = self.preview.as_mut() {
            preview.pause();
        }
        match draft {
            true => spawn_blocking(
                move || writing::keep(&file, span),
                move |editor: &mut Editor, outcome: Result<Saved>| {
                    editor.outcome(mark, outcome.map_err(Arc::new));
                },
            ),
            false => spawn_blocking(
                move || writing::write(&file, span),
                move |editor: &mut Editor, result: Result<()>| {
                    editor.saved(mark, result.map_err(Arc::new));
                },
            ),
        }
    }

    fn saved(&mut self, mark: u64, result: Result<(), Arc<Error>>) {
        if mark != self.opening {
            return skip_view();
        }
        self.working = false;
        match result {
            Ok(()) => {
                if let Some(opening) = self.pending_open.clone() {
                    self.take(opening);
                }
            }
            Err(error) => {
                tracing::warn!("failed to trim recording: {error}");
                self.trouble = Some(error);
            }
        }
    }

    fn outcome(&mut self, mark: u64, outcome: Result<Saved, Arc<Error>>) {
        if mark != self.opening {
            return skip_view();
        }
        self.working = false;
        match outcome {
            Ok(saved) => {
                Strings::now().notify_saved(&saved, &self.shared.seam);
                self.close();
            }
            Err(error) => {
                tracing::warn!("failed to save replay: {error}");
                self.trouble = Some(error);
            }
        }
    }

    pub(crate) fn close(&mut self) {
        self.release();
        self.forget();
        match self.shared.waiting.is_waiting() {
            true => self.arrived(),
            false => {
                self.shared.seam.notify(Request::new(CAPABILITY, CLOSE));
            }
        }
    }

    fn release(&self) {
        let Some(draft) = self
            .pending_open
            .as_ref()
            .filter(|pending_open| pending_open.draft)
        else {
            return;
        };
        let file = draft.file.clone();
        spawn_blocking(
            move || replays::discard(file),
            |_: &mut Editor, ()| skip_view(),
        );
    }

    fn forget(&mut self) {
        self.opening = self.opening.wrapping_add(1);
        cancel(WATCHING);
        self.preview = None;
        self.keyframes = None;
        self.cut = None;
        self.pending_open = None;
        self.trouble = None;
        self.working = false;
    }
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field(
                "file",
                &self
                    .pending_open
                    .as_ref()
                    .map(|pending_open| &pending_open.file),
            )
            .field("cut", &self.cut)
            .field("unsaved", &self.is_unsaved())
            .finish_non_exhaustive()
    }
}
