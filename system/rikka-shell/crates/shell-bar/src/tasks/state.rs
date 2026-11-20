use std::collections::HashMap;

use rikka_seam::{Payload, Reply, SeamClient, SeamError};
use rikka_session::toplevels::{self, ToplevelsClient};
use rikka_session::workspaces::{self, COUNT, WorkspacesClient};
use rikka_ui::spawn;
use shell_apps::Apps;
use task_board::{Act, Board, Group, Order};

use crate::tasks::tile::{Tile, owning_entry};

#[derive(Debug)]
pub(crate) struct Tasks {
    seam: SeamClient,
    apps: Apps,
    output: String,
    windows: toplevels::State,
    spaces: workspaces::State,
    order: Order,
    board: Board,
    tiles: HashMap<String, Tile>,
}

impl Tasks {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, apps: Apps, output: String) -> Tasks {
        Tasks {
            seam,
            apps,
            output,
            windows: toplevels::State::default(),
            spaces: workspaces::State::default(),
            order: Order::default(),
            board: Board::default(),
            tiles: HashMap::new(),
        }
    }

    #[must_use]
    pub(crate) fn seam(&self) -> &SeamClient {
        &self.seam
    }

    #[must_use]
    pub(crate) fn apps(&self) -> &Apps {
        &self.apps
    }

    #[must_use]
    pub(crate) fn output(&self) -> &str {
        &self.output
    }

    #[must_use]
    pub(crate) fn board(&self) -> &Board {
        &self.board
    }

    #[must_use]
    pub(crate) fn shown(&self) -> &[Group] {
        self.board
            .spaces
            .get(self.board.showing.saturating_sub(1))
            .map_or(&[], |space| space.groups.as_slice())
    }

    #[must_use]
    pub(crate) fn holding(&self) -> [bool; COUNT] {
        self.spaces
            .display(&self.output)
            .map_or([false; COUNT], |displayed| displayed.holding)
    }

    #[must_use]
    pub(crate) fn tiles(&self) -> &HashMap<String, Tile> {
        &self.tiles
    }

    #[must_use]
    pub(crate) fn group(&self, space: usize, key: &str) -> Option<(&Group, &Tile)> {
        let group = self.board.group(space, key)?;
        Some((group, self.tiles.get(key)?))
    }

    pub(crate) fn moved(&mut self, output: String) -> bool {
        let holding = self.holding();
        self.output = output;
        let settled = self.settle();
        settled || holding != self.holding()
    }

    pub(crate) fn windows_changed(&mut self, windows: toplevels::State) -> bool {
        self.windows = windows;
        self.settle()
    }

    pub(crate) fn spaces_changed(&mut self, spaces: workspaces::State) -> bool {
        let holding = self.holding();
        self.spaces = spaces;
        let settled = self.settle();
        settled || holding != self.holding()
    }

    pub(crate) fn settle(&mut self) -> bool {
        let showing = self
            .spaces
            .display(&self.output)
            .map_or(1, |displayed| displayed.showing);
        let (board, tiles) = self.apps.installed().with(|catalogue| {
            let entries = catalogue.entries();
            let board = Board::arrange(
                &self.windows.toplevels,
                &self.output,
                showing,
                &mut self.order,
                |toplevel| owning_entry(entries, toplevel),
            );
            let tiles: HashMap<String, Tile> = board
                .spaces
                .iter()
                .flat_map(|space| &space.groups)
                .map(|group| {
                    let tile = Tile::resolve(group, entries, self.apps.icons());
                    (group.key.clone(), tile)
                })
                .collect();
            (board, tiles)
        });
        let changed = !board.looks_like(&self.board) || tiles != self.tiles;
        self.board = board;
        self.tiles = tiles;
        changed
    }

    pub(crate) fn press(&self, space: usize, key: &str) {
        let Some(act) = self.board.group(space, key).and_then(Group::press) else {
            return;
        };
        let toplevels = self.toplevels();
        ask(match act {
            Act::Focus(id) => toplevels.focus(id),
            Act::Minimize(id) => toplevels.minimize(id),
        });
    }

    pub(crate) fn launch(&self, key: &str) {
        if let Some(spawn) = self.tiles.get(key).and_then(Tile::starts) {
            ask(self.seam.call(spawn.request()));
        }
    }

    pub(crate) fn focus(&self, id: &str) {
        ask(self.toplevels().focus(id));
    }

    pub(crate) fn close(&self, id: &str) {
        ask(self.toplevels().close(id));
    }

    pub(crate) fn show(&self, number: usize) {
        ask(WorkspacesClient::new(self.seam.clone()).show(Some(self.output.clone()), number));
    }

    fn toplevels(&self) -> ToplevelsClient {
        ToplevelsClient::new(self.seam.clone())
    }
}

pub(crate) fn ask(reply: Reply) {
    spawn(reply, |reply: Result<Payload, SeamError>| {
        if let Err(error) = reply {
            tracing::warn!("taskbar request failed: {error}");
        }
    });
}
