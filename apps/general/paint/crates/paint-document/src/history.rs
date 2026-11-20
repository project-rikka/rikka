use rikka_image_edit::Operation;

use crate::edit::{FigureChange, SelectionAction, StackOrder};
use crate::error::Result;
use crate::figure::FigureKind;
use crate::layer::LayerId;
use crate::reductions::DirtyRegion;
use crate::reshape::Reshape;
use crate::state::{State, Swap};

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Open,
    Show { layer: LayerId, visible: bool },
    Select(SelectionAction),
    Crop,
    Develop(Operation),
    Fill,
    Erase,
    Reshape(Reshape),
    Convert,
    Bucket,
    Figure(FigureAction),
    Batch(String),
    New,
    NewLayer,
    DuplicateLayer,
    DeleteLayer,
    MoveLayer,
    RenameLayer,
    LayerOpacity,
    LayerBlend,
    LayerProperties,
    MergeDown,
    MergeVisible,
    Flatten,
    LayerFromBackground,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FigureAction {
    Draw(FigureKind),
    Paste,
    Duplicate,
    Change(FigureChange),
    Delete,
    Stack(StackOrder),
    Rasterize,
}

#[derive(Clone, Copy, Debug)]
pub struct History<'a> {
    actions: &'a [Action],
    current: usize,
    saved: Option<usize>,
}

impl<'a> History<'a> {
    #[must_use]
    pub fn actions(&self) -> &'a [Action] {
        self.actions
    }

    #[must_use]
    pub fn current(&self) -> usize {
        self.current
    }

    #[must_use]
    pub fn saved(&self) -> Option<usize> {
        self.saved
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.current > 0
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.current + 1 < self.actions.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Limits {
    pub(crate) states: usize,
    pub(crate) bytes: u64,
}

#[derive(Debug)]
pub(crate) struct Timeline {
    actions: Vec<Action>,
    entries: Vec<Entry>,
    current: usize,
    saved: Option<usize>,
    total_bytes: u64,
    limits: Limits,
}

#[derive(Debug, Default)]
struct Entry {
    swaps: Vec<Swap>,
    bytes: u64,
    focus: Focus,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Focus {
    pub(crate) before: Option<LayerId>,
    pub(crate) after: Option<LayerId>,
}

impl Focus {
    pub(crate) fn on(layer: LayerId) -> Focus {
        Focus {
            before: Some(layer),
            after: Some(layer),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Travelled {
    pub(crate) look: bool,
    pub(crate) selection: bool,
    pub(crate) layer: Option<LayerId>,
    pub(crate) touched: DirtyRegion,
}

impl Travelled {
    fn and(self, other: Travelled) -> Travelled {
        Travelled {
            look: self.look || other.look,
            selection: self.selection || other.selection,
            layer: self.layer.or(other.layer),
            touched: self.touched.and(other.touched),
        }
    }

    fn focused(self, layer: Option<LayerId>) -> Travelled {
        Travelled { layer, ..self }
    }
}

impl Entry {
    fn exchange(&mut self, state: &mut State, backward: bool) -> Result<u64> {
        let order: Vec<usize> = match backward {
            true => (0..self.swaps.len()).rev().collect(),
            false => (0..self.swaps.len()).collect(),
        };
        for (done, &index) in order.iter().enumerate() {
            if let Err(error) = self.swaps[index].exchange(state) {
                for &undone in order[..done].iter().rev() {
                    let _ = self.swaps[undone].exchange(state);
                }
                return Err(error);
            }
        }
        let before = self.bytes;
        self.bytes = self.swaps.iter().map(Swap::bytes).sum();
        Ok(before)
    }

    fn moved(&self) -> Travelled {
        self.swaps.iter().fold(Travelled::default(), |moved, swap| {
            let selection = matches!(swap, Swap::Selection(_));
            Travelled {
                look: moved.look || !selection,
                selection: moved.selection || selection,
                layer: None,
                touched: moved.touched.and(swap.touched()),
            }
        })
    }
}

impl Timeline {
    pub(crate) fn new(first: Action, limits: Limits) -> Timeline {
        Timeline {
            actions: vec![first],
            entries: vec![Entry::default()],
            current: 0,
            saved: Some(0),
            total_bytes: 0,
            limits,
        }
    }

    pub(crate) fn view(&self) -> History<'_> {
        History {
            actions: &self.actions,
            current: self.current,
            saved: self.saved,
        }
    }

    pub(crate) fn record(&mut self, action: Action, swaps: Vec<Swap>, focus: Focus) {
        let keep_until = self.current + 1;
        self.total_bytes -= self
            .entries
            .drain(keep_until..)
            .map(|entry| entry.bytes)
            .sum::<u64>();
        self.actions.truncate(keep_until);
        if self.saved.is_some_and(|saved| saved >= keep_until) {
            self.saved = None;
        }
        let bytes = swaps.iter().map(Swap::bytes).sum();
        self.total_bytes += bytes;
        self.entries.push(Entry {
            swaps,
            bytes,
            focus,
        });
        self.actions.push(action);
        self.current = keep_until;
        self.trim();
    }

    pub(crate) fn undo(&mut self, state: &mut State) -> Result<Option<Travelled>> {
        if self.current == 0 {
            return Ok(None);
        }
        let before = self.entries[self.current].focus.before;
        let moved = self.exchange(self.current, state, true)?.focused(before);
        self.current -= 1;
        Ok(Some(moved))
    }

    pub(crate) fn redo(&mut self, state: &mut State) -> Result<Option<Travelled>> {
        if self.current + 1 >= self.entries.len() {
            return Ok(None);
        }
        let after = self.entries[self.current + 1].focus.after;
        let moved = self
            .exchange(self.current + 1, state, false)?
            .focused(after);
        self.current += 1;
        Ok(Some(moved))
    }

    pub(crate) fn revisit(&mut self, index: usize, state: &mut State) -> Result<Option<Travelled>> {
        let target = index.min(self.entries.len() - 1);
        let mut moved = None;
        while self.current != target {
            let step = match self.current > target {
                true => self.undo(state)?,
                false => self.redo(state)?,
            };
            moved = step.map(|step| step.and(moved.unwrap_or_default()));
        }
        Ok(moved)
    }

    pub(crate) fn mark_saved(&mut self) {
        self.saved = Some(self.current);
    }

    pub(crate) fn is_modified(&self) -> bool {
        self.saved != Some(self.current)
    }

    fn exchange(&mut self, index: usize, state: &mut State, backward: bool) -> Result<Travelled> {
        let entry = &mut self.entries[index];
        let before = entry.exchange(state, backward)?;
        self.total_bytes = self.total_bytes - before + entry.bytes;
        Ok(entry.moved())
    }

    fn trim(&mut self) {
        while self.current > 0
            && (self.entries.len() > self.limits.states || self.total_bytes > self.limits.bytes)
        {
            self.entries.remove(0);
            self.actions.remove(0);
            let base = std::mem::take(&mut self.entries[0]);
            self.total_bytes -= base.bytes;
            self.current -= 1;
            self.saved = self.saved.and_then(|saved| saved.checked_sub(1));
        }
    }
}
