use std::rc::Rc;

use rikka_types::{Key, Modifiers};
use rikka_ui::{
    Behavior, Command, Element, Event, EventCtx, Handler, IntoHandler, Layout, Length, View,
    Widget, skip_view,
};

type Resolve<C> = Rc<dyn Fn(Modifiers, Key) -> Option<C>>;
type Standard<C> = Rc<dyn Fn(Command) -> Option<C>>;

#[must_use]
pub struct Shortcuts<C> {
    child: Element,
    pressed: Resolve<C>,
    standard: Option<Standard<C>>,
    run: Handler<(C,)>,
    hold_binding: Option<(Key, Handler<(bool,)>)>,
    paused: bool,
}

pub fn shortcuts<S: 'static, C: Copy + 'static>(
    child: impl View,
    pressed: impl Fn(Modifiers, Key) -> Option<C> + 'static,
    enabled: impl Fn(C, &S) -> bool + 'static,
    run: impl Fn(C, &mut S) + 'static,
) -> Shortcuts<C> {
    let run = move |host: &mut S, command: C| match enabled(command, host) {
        true => run(command, host),
        false => skip_view(),
    };
    Shortcuts {
        child: child.build(),
        pressed: Rc::new(pressed),
        standard: None,
        run: run.into_handler(),
        hold_binding: None,
        paused: false,
    }
}

impl<C: Copy + 'static> Shortcuts<C> {
    pub fn standard(mut self, of: impl Fn(Command) -> Option<C> + 'static) -> Self {
        self.standard = Some(Rc::new(of));
        self
    }

    pub fn holding<Marker>(
        mut self,
        key: Key,
        hold: impl IntoHandler<(bool,), (), Marker>,
    ) -> Self {
        self.hold_binding = Some((key, hold.into_handler()));
        self
    }

    pub fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }

    fn hold(&self, key: Key, down: bool, ctx: &mut EventCtx<'_>) -> bool {
        let Some((hold_key, hold)) = &self.hold_binding else {
            return false;
        };
        if *hold_key != key {
            return false;
        }
        ctx.set_handled();
        ctx.call(hold, (down,));
        true
    }

    fn dispatch(&self, command: Option<C>, ctx: &mut EventCtx<'_>) {
        if let Some(command) = command {
            ctx.set_handled();
            ctx.call(&self.run, (command,));
        }
    }
}

impl<C: Copy + 'static> Widget for Shortcuts<C> {
    type State = ();

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn event(&self, _state: &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::KeyPressed { key, modifiers, .. }
                if modifiers == Modifiers::NONE && self.hold(key, true, ctx) => {}
            Event::KeyReleased { key, .. } if self.hold(key, false, ctx) => {}
            Event::FocusLost => {
                if let Some((_, hold)) = &self.hold_binding {
                    ctx.call(hold, (false,));
                }
            }
            Event::KeyPressed { key, modifiers, .. } => {
                self.dispatch((self.pressed)(modifiers, key), ctx);
            }
            Event::Command(standard) => {
                let command = self.standard.as_ref().and_then(|of| of(standard));
                self.dispatch(command, ctx);
            }
            _ => {}
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.paused == other.paused
    }

    fn behavior(&self, _state: &()) -> Behavior {
        match self.paused {
            true => Behavior::INERT,
            false => Behavior::FOCUSABLE | Behavior::AUTOFOCUS | Behavior::NO_TAB_STOP,
        }
    }

    fn debug_name(&self) -> &'static str {
        "Shortcuts"
    }
}

impl<C> std::fmt::Debug for Shortcuts<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shortcuts")
            .field("paused", &self.paused)
            .finish_non_exhaustive()
    }
}

impl<C: Copy + 'static> View for Shortcuts<C> {
    fn build(mut self) -> Element {
        let child = std::mem::replace(&mut self.child, Element::empty());
        Element::with_children(self, vec![child])
    }
}
