use std::cell::RefCell;
use std::rc::Rc;

use rikka_session::toplevels::State;
use rikka_types::{Point, Rect};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Window {
    pub(crate) id: String,
    pub(crate) bounds: Rect,
}

#[derive(Debug, Default)]
struct Open {
    windows: Vec<Window>,
    focused_output: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Windows(Rc<RefCell<Open>>);

impl Windows {
    pub(crate) fn follow(&self, state: &State) {
        *self.0.borrow_mut() = Open {
            windows: state
                .toplevels
                .iter()
                .filter_map(|window| {
                    let bounds = window.bounds;
                    (window.on_screen && bounds.area() > 0).then(|| Window {
                        id: window.id.clone(),
                        bounds: Rect::new(
                            bounds.x as f32,
                            bounds.y as f32,
                            bounds.width as f32,
                            bounds.height as f32,
                        ),
                    })
                })
                .collect(),
            focused_output: state
                .toplevels
                .iter()
                .find(|window| window.focused)
                .map(|window| window.output.clone()),
        };
    }

    #[must_use]
    pub(crate) fn at(&self, point: Point) -> Option<Window> {
        self.0
            .borrow()
            .windows
            .iter()
            .rev()
            .find(|window| window.bounds.contains(point))
            .cloned()
    }

    #[must_use]
    pub(super) fn focused_output(&self) -> Option<String> {
        self.0.borrow().focused_output.clone()
    }
}
