use std::any::TypeId;

use rikka_ui::{Element, State, StateRef, Ui, Update, View, WindowChrome};
use rikka_widgets::{CaptionBar, window_frame};

pub(crate) fn set_chrome(ui: &mut Ui, chrome: WindowChrome) {
    ui.send(Update::new(|framing: &mut Framing| framing.chrome = chrome));
}

#[cfg(unix)]
pub(crate) fn set_client_side(ui: &mut Ui, client_side: bool) {
    ui.send(Update::new(move |framing: &mut Framing| {
        framing.client_side = client_side;
    }));
}

pub(crate) struct Decorated<S> {
    inner: S,
    framing: Framing,
}

struct Framing {
    chrome: WindowChrome,
    client_side: bool,
}

impl<S: State> Decorated<S> {
    pub(crate) fn new(inner: S, client_side: bool) -> Self {
        Self {
            inner,
            framing: Framing {
                chrome: WindowChrome::default(),
                client_side,
            },
        }
    }

    pub(crate) fn inner(&self) -> &S {
        &self.inner
    }

    pub(crate) fn frame(
        &self,
        content: Element,
        caption: impl FnOnce(&S) -> Option<CaptionBar>,
    ) -> Element {
        if !self.framing.client_side {
            return content;
        }
        let frame = window_frame(content).chrome(self.framing.chrome.clone());
        let caption = match self.framing.chrome.fullscreen {
            true => None,
            false => caption(&self.inner),
        };
        match caption {
            Some(caption) => frame.caption(caption).build(),
            None => frame.build(),
        }
    }
}

impl<S: State> State for Decorated<S> {
    fn holds(&self, target: TypeId) -> usize {
        self.inner.holds(target) + usize::from(target == TypeId::of::<Framing>())
    }

    fn reach(&mut self, target: TypeId) -> Option<StateRef<'_>> {
        match target == TypeId::of::<Framing>() {
            true => Some(StateRef::new(&mut self.framing, true)),
            false => self.inner.reach(target),
        }
    }
}

impl<S> std::fmt::Debug for Decorated<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Decorated")
            .field("chrome", &self.framing.chrome)
            .field("client_side", &self.framing.client_side)
            .finish_non_exhaustive()
    }
}
