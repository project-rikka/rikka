use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rikka_types::Choice;

use crate::Shell;
use crate::appearance::{self, TaskbarPosition};
use crate::taskbar::Shape;

type Listener = Box<dyn Fn(Option<Shape>)>;

#[derive(Clone)]
pub struct TaskbarLayout(Rc<Inner>);

struct Inner {
    up: Cell<bool>,
    position: Cell<TaskbarPosition>,
    listeners: RefCell<Vec<Listener>>,
}

impl TaskbarLayout {
    #[must_use]
    pub fn read(shell: &Shell) -> TaskbarLayout {
        TaskbarLayout::new(
            appearance::taskbar(shell),
            appearance::taskbar_position(shell),
        )
    }

    fn new(up: bool, position: TaskbarPosition) -> TaskbarLayout {
        TaskbarLayout(Rc::new(Inner {
            up: Cell::new(up),
            position: Cell::new(position),
            listeners: RefCell::default(),
        }))
    }

    #[must_use]
    pub fn shape(&self) -> Option<Shape> {
        self.0.up.get().then(|| Shape::new(self.0.position.get()))
    }

    #[must_use]
    pub fn following(&self, shell: Shell, moved: impl Fn(Option<Shape>) + 'static) -> Shell {
        self.0.listeners.borrow_mut().push(Box::new(moved));
        shell
            .on_setting(appearance::DOMAIN, appearance::TASKBAR, {
                let taskbar_layout = self.clone();
                move |_, up: bool| taskbar_layout.alter(|inner| inner.up.replace(up) != up)
            })
            .on_setting(appearance::DOMAIN, appearance::TASKBAR_POSITION, {
                let taskbar_layout = self.clone();
                move |_, slug: String| {
                    let position = TaskbarPosition::of_slug(&slug);
                    taskbar_layout.alter(|inner| inner.position.replace(position) != position);
                }
            })
    }

    fn alter(&self, change: impl FnOnce(&Inner) -> bool) {
        if !change(&self.0) {
            return;
        }
        let shape = self.shape();
        for listener in self.0.listeners.borrow().iter() {
            listener(shape);
        }
    }
}

impl std::fmt::Debug for TaskbarLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TaskbarLayout").field(&self.shape()).finish()
    }
}
