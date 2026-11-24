use std::cell::Cell;
use std::rc::Rc;

use desk_layout::Area;
use rikka_types::{Point, Rect};
use rikka_ui::{Observed, Observers};
use shell_kit::Shell;
use shell_kit::taskbar::{Shape, TaskbarLayout};

#[derive(Clone, Default)]
pub struct Edges(Rc<Inner>);

#[derive(Default)]
struct Inner {
    taskbar: Cell<Option<Shape>>,
    observers: Observers,
}

impl Edges {
    #[must_use]
    pub(crate) fn follow(&self, shell: Shell) -> Shell {
        let taskbar_layout = TaskbarLayout::read(&shell);
        self.0.taskbar.set(taskbar_layout.shape());
        taskbar_layout.following(shell, {
            let edges = self.clone();
            move |shape| edges.alter(shape)
        })
    }

    #[must_use]
    pub fn taskbar(&self) -> Option<Shape> {
        self.0.observers.read();
        self.0.taskbar.get()
    }

    #[must_use]
    pub(crate) fn taskbar_covers(&self, output: Rect, at: Point) -> bool {
        self.taskbar().is_some_and(|shape| shape.covers(output, at))
    }

    #[must_use]
    pub(crate) fn area(&self, output: Rect) -> Area {
        self.0.observers.read();
        let desktop = self
            .0
            .taskbar
            .get()
            .map_or(output, |shape| shape.desktop(output).rect);
        Area::for_desk(desktop)
    }

    fn alter(&self, taskbar: Option<Shape>) {
        if self.0.taskbar.replace(taskbar) != taskbar {
            self.0.observers.changed();
        }
    }
}

impl Observed for Edges {
    fn observers(&self) -> &Observers {
        &self.0.observers
    }
}

impl std::fmt::Debug for Edges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Edges")
            .field("taskbar", &self.0.taskbar.get())
            .finish_non_exhaustive()
    }
}
