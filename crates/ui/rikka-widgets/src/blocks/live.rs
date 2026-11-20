use rikka_ui::{Element, Layout, NodeHandle, Style, View, Watchers, Widget};

#[derive(Widget)]
pub struct Live {
    watchers: Option<Watchers>,
    view: Box<dyn Fn() -> Element>,
    #[style]
    style: Style,
}

pub fn live<V: View>(watchers: &Watchers, view: impl Fn() -> V + 'static) -> Live {
    Live {
        watchers: Some(watchers.clone()),
        view: Box::new(move || view().build()),
        style: Style::default(),
    }
}

pub fn live_observed<V: View>(view: impl Fn() -> V + 'static) -> Live {
    Live {
        watchers: None,
        view: Box::new(move || view().build()),
        style: Style::default(),
    }
}

impl Widget for Live {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().with(&self.style)
    }

    fn build(&self, (): &mut ()) -> Vec<Element> {
        if let Some(watchers) = &self.watchers {
            watchers.watch(NodeHandle::current());
        }
        vec![(self.view)()]
    }
}
