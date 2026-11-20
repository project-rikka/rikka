use rikka_session::toplevels::{Bounds, Toplevel};
use rikka_session::workspaces::{COUNT, exists};

use crate::order::Order;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub id: String,
    pub title: String,
    pub focused: bool,
    pub minimized: bool,
    pub maximized: bool,
    pub tiled: bool,
    pub bounds: Bounds,
    pub(crate) recency: usize,
}

impl Window {
    fn new(toplevel: &Toplevel, recency: usize) -> Window {
        Window {
            id: toplevel.id.clone(),
            title: toplevel.title.clone(),
            focused: toplevel.focused,
            minimized: toplevel.minimized,
            maximized: toplevel.maximized,
            tiled: toplevel.tiled,
            bounds: toplevel.bounds,
            recency,
        }
    }

    fn looks_like(&self, other: &Window) -> bool {
        self.id == other.id && self.minimized == other.minimized
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub key: String,
    pub app_id: String,
    pub windows: Vec<Window>,
}

impl Group {
    #[must_use]
    pub fn is_focused(&self) -> bool {
        self.windows.iter().any(|window| window.focused)
    }

    #[must_use]
    pub fn is_minimized(&self) -> bool {
        self.windows.iter().all(|window| window.minimized)
    }

    fn looks_like(&self, other: &Group) -> bool {
        self.key == other.key
            && self.app_id == other.app_id
            && self.windows.len() == other.windows.len()
            && self
                .windows
                .iter()
                .zip(&other.windows)
                .all(|(was, is)| was.looks_like(is))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Space {
    pub number: usize,
    pub groups: Vec<Group>,
}

impl Space {
    fn looks_like(&self, other: &Space) -> bool {
        self.number == other.number
            && self.groups.len() == other.groups.len()
            && self
                .groups
                .iter()
                .zip(&other.groups)
                .all(|(was, is)| was.looks_like(is))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub showing: usize,
    pub spaces: Vec<Space>,
}

impl Default for Board {
    fn default() -> Board {
        Board {
            showing: 1,
            spaces: (1..=COUNT)
                .map(|number| Space {
                    number,
                    groups: Vec::new(),
                })
                .collect(),
        }
    }
}

impl Board {
    #[must_use]
    pub fn arrange(
        toplevels: &[Toplevel],
        output: &str,
        showing: usize,
        order: &mut Order,
        owner: impl Fn(&Toplevel) -> Option<String>,
    ) -> Board {
        let here: Vec<(usize, &Toplevel, String)> = toplevels
            .iter()
            .enumerate()
            .filter(|(_, toplevel)| toplevel.output == output && exists(toplevel.workspace))
            .map(|(recency, toplevel)| {
                let key = owner(toplevel).unwrap_or_else(|| toplevel.id.clone());
                (recency, toplevel, key)
            })
            .collect();
        order.settle_groups(
            here.iter()
                .map(|(_, toplevel, key)| (toplevel.workspace, key.as_str())),
        );
        order.settle_windows(here.iter().map(|(_, toplevel, _)| toplevel.id.as_str()));

        let mut board = Board {
            showing,
            ..Board::default()
        };
        for (recency, toplevel, key) in &here {
            let space = &mut board.spaces[toplevel.workspace - 1];
            let window = Window::new(toplevel, *recency);
            match space.groups.iter_mut().find(|group| group.key == *key) {
                Some(group) => group.windows.push(window),
                None => space.groups.push(Group {
                    key: key.clone(),
                    app_id: toplevel.app_id.clone(),
                    windows: vec![window],
                }),
            }
        }
        for space in &mut board.spaces {
            let number = space.number;
            space
                .groups
                .sort_by_key(|group| order.group_place(number, &group.key));
            for group in &mut space.groups {
                group
                    .windows
                    .sort_by_key(|window| order.window_place(&window.id));
            }
        }
        board
    }

    #[must_use]
    pub fn group(&self, space: usize, key: &str) -> Option<&Group> {
        self.spaces
            .get(space.checked_sub(1)?)?
            .groups
            .iter()
            .find(|group| group.key == key)
    }

    #[must_use]
    pub fn looks_like(&self, other: &Board) -> bool {
        self.showing == other.showing
            && self.spaces.len() == other.spaces.len()
            && self
                .spaces
                .iter()
                .zip(&other.spaces)
                .all(|(was, is)| was.looks_like(is))
    }
}
