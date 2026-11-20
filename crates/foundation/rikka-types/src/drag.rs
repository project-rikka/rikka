use std::ops::BitOr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragAction {
    Copy,
    Move,
    Ask,
}

impl DragAction {
    pub const ALL: [Self; 3] = [Self::Copy, Self::Move, Self::Ask];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DragActions {
    copy: bool,
    moves: bool,
    ask: bool,
}

impl DragActions {
    pub const NONE: Self = Self {
        copy: false,
        moves: false,
        ask: false,
    };
    pub const COPY: Self = Self {
        copy: true,
        moves: false,
        ask: false,
    };
    pub const MOVE: Self = Self {
        copy: false,
        moves: true,
        ask: false,
    };
    pub const ASK: Self = Self {
        copy: false,
        moves: false,
        ask: true,
    };
    pub const COPY_OR_MOVE: Self = Self {
        copy: true,
        moves: true,
        ask: false,
    };

    #[must_use]
    pub fn contains(self, action: DragAction) -> bool {
        match action {
            DragAction::Copy => self.copy,
            DragAction::Move => self.moves,
            DragAction::Ask => self.ask,
        }
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self == Self::NONE
    }
}

impl From<DragAction> for DragActions {
    fn from(action: DragAction) -> Self {
        Self {
            copy: action == DragAction::Copy,
            moves: action == DragAction::Move,
            ask: action == DragAction::Ask,
        }
    }
}

impl BitOr for DragActions {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self {
            copy: self.copy || other.copy,
            moves: self.moves || other.moves,
            ask: self.ask || other.ask,
        }
    }
}
