use std::time::Duration;

use futures_util::StreamExt;
use rikka_session::workspaces::COUNT;
use rikka_types::Rect;
use rikka_ui::{State, every_keyed, skip_view, stream};
use shell_kit::Placement;

use crate::indicators::Kind;
use crate::island::Island;
use crate::shared::Shared;
use crate::status::Bell;
use crate::tasks::Tasks;

const INDICATING: &str = "taskbar.indicators";

const INDICATORS_EVERY: Duration = Duration::from_secs(30);

#[derive(Debug, State)]
pub(crate) struct Bar {
    shared: Shared,
    hovered: bool,
    #[state]
    island: Island,
}

impl Bar {
    #[must_use]
    pub(crate) fn new(shared: Shared, placement: &Placement) -> Bar {
        let tasks = Tasks::new(
            shared.seam.clone(),
            shared.apps.clone(),
            placement.name.clone(),
        );
        let island = Island::new(
            shared.feeds.clone(),
            shared.recording.clone(),
            tasks,
            shared.menu.clone(),
            placement.size.width,
        );
        Bar {
            shared,
            hovered: false,
            island,
        }
    }

    pub(crate) fn start(&mut self) {
        let daemons = &self.shared.daemons;
        stream(daemons.sound().changes().map(|_| ()), moved);
        stream(daemons.network().changes().map(|_| ()), moved);
        stream(daemons.battery().main_changes().map(|_| ()), moved);
        stream(self.shared.tray.changes().map(|_| ()), moved);
        stream(self.shared.fold.changes().map(|_| ()), moved);
        stream(self.shared.apps.installed().changes(), moved);
        stream(self.shared.history.changes(), moved);
        stream(self.shared.quiet.changes().map(|_| ()), moved);
        stream(self.shared.shelf.changes(), moved);
        stream(self.shared.percentage.changes(), moved);
        self.shared.shelf.tick();
        every_keyed(INDICATING, INDICATORS_EVERY, Bar::tick_indicators);
        self.island.start();
    }

    pub(crate) fn moved(&mut self, placement: Placement) {
        self.island.moved(&placement);
    }

    pub(crate) fn shared(&self) -> &Shared {
        &self.shared
    }

    pub(super) fn is_hovered(&self) -> bool {
        self.hovered
    }

    pub(crate) fn island(&self) -> &Island {
        &self.island
    }

    pub(crate) fn bell(&self) -> Option<Bell> {
        Bell::new(self.shared.quiet.get(), self.shared.history.is_unread())
    }

    pub(super) fn stop_recording(&mut self) {
        self.shared.stop_recording();
    }

    pub(crate) fn show(&mut self, number: usize) {
        self.island.tasks().show(number);
        skip_view();
    }

    pub(crate) fn showing(&self) -> usize {
        self.island.tasks().board().showing
    }

    pub(crate) fn holding(&self) -> [bool; COUNT] {
        self.island.tasks().holding()
    }

    pub(crate) fn hover(&mut self, over: bool) {
        self.hovered = over;
    }

    pub(crate) fn open(&mut self) {
        self.shared.open();
    }

    pub(super) fn assist(&mut self) {
        self.shared.assist();
        skip_view();
    }

    pub(crate) fn unfold(&mut self, chevron: Rect) {
        self.shared.fold.unfold(chevron);
    }

    pub(super) fn indicate(&mut self, kind: Kind, at: Rect) {
        self.shared.shelf.open(kind, at);
    }

    fn tick_indicators(&mut self) {
        self.shared.shelf.tick();
        skip_view();
    }
}

fn moved(_: &mut Bar, (): ()) {}
