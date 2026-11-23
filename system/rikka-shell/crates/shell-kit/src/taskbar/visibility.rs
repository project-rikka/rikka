use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::taskbar::Shape;
use crate::{Shell, appearance};

type Listener = Box<dyn Fn(bool)>;

#[derive(Clone)]
pub struct TaskbarVisibility(Rc<Inner>);

struct Inner {
    up: Cell<bool>,
    listeners: RefCell<Vec<Listener>>,
}

impl TaskbarVisibility {
    #[must_use]
    pub fn read(shell: &Shell) -> TaskbarVisibility {
        TaskbarVisibility::up(appearance::taskbar(shell))
    }

    fn up(up: bool) -> TaskbarVisibility {
        TaskbarVisibility(Rc::new(Inner {
            up: Cell::new(up),
            listeners: RefCell::default(),
        }))
    }

    #[must_use]
    pub fn is_up(&self) -> bool {
        self.0.up.get()
    }

    #[must_use]
    pub fn shape(&self) -> Option<Shape> {
        self.is_up().then_some(Shape)
    }

    #[must_use]
    pub fn below(&self) -> f32 {
        self.shape().map_or(0.0, Shape::reserved)
    }

    #[must_use]
    pub fn border(&self) -> f32 {
        self.shape().map_or(0.0, Shape::border)
    }

    fn on_move(&self, listener: impl Fn(bool) + 'static) {
        self.0.listeners.borrow_mut().push(Box::new(listener));
    }

    #[must_use]
    pub fn following(&self, shell: Shell, moved: impl Fn(bool) + 'static) -> Shell {
        self.on_move(moved);
        shell.on_setting(appearance::DOMAIN, appearance::TASKBAR, {
            let taskbar_visibility = self.clone();
            move |_, up: bool| taskbar_visibility.set_shown(up)
        })
    }

    fn set_shown(&self, up: bool) {
        if self.0.up.replace(up) == up {
            return;
        }
        for listener in self.0.listeners.borrow().iter() {
            listener(up);
        }
    }
}

impl std::fmt::Debug for TaskbarVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TaskbarVisibility")
            .field(&self.is_up())
            .finish()
    }
}

impl Default for TaskbarVisibility {
    fn default() -> TaskbarVisibility {
        TaskbarVisibility::up(appearance::TASKBAR_DEFAULT)
    }
}
