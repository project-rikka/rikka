use rikka_types::{Color, CornerRadii, Gradient, Key, Modifiers, RoundedRect};
use rikka_ui::{
    Action, Align, Behavior, Command, Corner, Element, Event, EventCtx, Handler, Inset,
    IntoHandler, Layout, PaintCtx, Quad, Reveal, Shadow, Style, Styled, View, Widget, motion,
};

use crate::blocks::springing::spring_height;
use crate::controls::press::Press;
use crate::controls::ripple::Ripple;

#[must_use]
pub struct Container {
    child: Option<Element>,
    padding: Inset,
    surface: bool,
    background: Option<Color>,
    background_gradient: Option<Box<Gradient>>,
    radius: Option<CornerRadii>,
    border: Option<(f32, Color)>,
    border_gradient: Option<(f32, Box<Gradient>)>,
    shadows: Vec<Shadow>,
    ground: Ground,
    resizing: Option<Align>,
    clips: bool,
    align_x: Option<Align>,
    align_y: Option<Align>,
    press: Action,
    on_hover: Action<(bool,)>,
    focusless: bool,
    reveal: Option<Reveal>,
    modal: bool,
    chords: Vec<Chord>,
    commands: Vec<(Command, Handler)>,
    style: Style,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Ground {
    #[default]
    Plain,
    Glass,
    Pane,
}

struct Chord {
    key: Key,
    modifiers: Modifiers,
    handler: Handler,
}

pub fn container(child: impl View) -> Container {
    Container {
        child: Some(child.build()),
        padding: Inset::NONE,
        surface: false,
        background: None,
        background_gradient: None,
        radius: None,
        border: None,
        border_gradient: None,
        shadows: Vec::new(),
        ground: Ground::default(),
        resizing: None,
        clips: false,
        align_x: None,
        align_y: None,
        press: Action::default(),
        on_hover: Action::default(),
        focusless: false,
        reveal: None,
        modal: false,
        chords: Vec::new(),
        commands: Vec::new(),
        style: Style::default(),
    }
}

impl Container {
    pub fn padding(mut self, padding: impl Into<Inset>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self.background_gradient = None;
        self
    }

    pub fn background_gradient(mut self, gradient: Gradient) -> Self {
        self.background_gradient = Some(Box::new(gradient));
        self.background = None;
        self
    }

    pub fn surface(mut self) -> Self {
        self.surface = true;
        self
    }

    pub fn radius(mut self, radius: impl Into<CornerRadii>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.border = Some((width, color));
        self.border_gradient = None;
        self
    }

    pub fn border_gradient(mut self, width: f32, gradient: Gradient) -> Self {
        self.border_gradient = Some((width, Box::new(gradient)));
        self.border = None;
        self
    }

    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.shadows.push(shadow);
        self
    }

    pub fn glass(mut self) -> Self {
        self.ground = Ground::Glass;
        self
    }

    pub fn pane(mut self) -> Self {
        self.ground = Ground::Pane;
        self
    }

    pub fn resizing(mut self) -> Self {
        self.resizing = Some(Align::Start);
        self
    }

    pub fn resizing_upward(mut self) -> Self {
        self.resizing = Some(Align::End);
        self
    }

    pub fn clip_content(mut self) -> Self {
        self.clips = true;
        self
    }

    pub fn center(self) -> Self {
        self.align_x(Align::Center).align_y(Align::Center)
    }

    pub fn align_x(mut self, align: Align) -> Self {
        self.align_x = Some(align);
        self
    }

    pub fn align_y(mut self, align: Align) -> Self {
        self.align_y = Some(align);
        self
    }

    pub fn on_press<Marker>(mut self, press: impl IntoHandler<(), (), Marker>) -> Self {
        self.press = Action::new(press);
        self
    }

    pub fn on_hover<Marker>(mut self, on_hover: impl IntoHandler<(bool,), (), Marker>) -> Self {
        self.on_hover = Action::new(on_hover);
        self
    }

    pub fn focusless(mut self) -> Self {
        self.focusless = true;
        self
    }

    pub(crate) fn modal(mut self) -> Self {
        self.modal = true;
        self
    }

    pub fn reveal(mut self, reveal: impl Into<Option<Reveal>>) -> Self {
        self.reveal = reveal.into();
        self
    }

    pub fn on_chord<Marker>(
        mut self,
        key: Key,
        modifiers: Modifiers,
        handler: impl IntoHandler<(), (), Marker>,
    ) -> Self {
        let key = key.to_lowercase();
        self.chords
            .retain(|chord| chord.key != key || chord.modifiers != modifiers);
        self.chords.push(Chord {
            key,
            modifiers,
            handler: handler.into_handler(),
        });
        self
    }

    pub fn on_command<Marker>(
        mut self,
        command: Command,
        handler: impl IntoHandler<(), (), Marker>,
    ) -> Self {
        self.commands.retain(|(other, _)| *other != command);
        self.commands.push((command, handler.into_handler()));
        self
    }

    pub(crate) fn with_layout(mut self, layout: rikka_ui::Style) -> Self {
        self.style = layout;
        self
    }

    fn border_width(&self) -> f32 {
        match (&self.border, &self.border_gradient) {
            (Some((width, _)), _) | (None, Some((width, _))) => *width,
            (None, None) => 0.0,
        }
    }

    fn resize(&self, state: &mut ContainerState, ctx: &mut PaintCtx<'_>) {
        if self.resizing.is_none() {
            return;
        }
        let target_height = ctx.child_rect(0).map_or(ctx.bounds().height(), |child| {
            child.height() + self.padding.vertical()
        });
        spring_height(ctx, &mut state.shown, target_height, motion::RESHAPE);
    }
}

#[derive(Debug, Default)]
pub struct ContainerState {
    shown: Option<f32>,
    revealed: bool,
    press: Press,
    ripple: Ripple,
}

impl Widget for Container {
    type State = ContainerState;

    fn layout(&self) -> Layout {
        let mut layout = Layout {
            justify: self.align_x.map(Align::to_justify),
            align_items: self.align_y.or(self.resizing),
            ..Layout::DEFAULT
        }
        .padding(self.padding);
        self.style.apply(&mut layout);
        layout
    }

    fn settled_height(&self, state: &ContainerState) -> Option<f32> {
        self.resizing.and(state.shown)
    }

    fn paint(&self, state: &mut ContainerState, ctx: &mut PaintCtx<'_>) {
        if let Some(reveal) = self.reveal
            && !state.revealed
        {
            ctx.reveal(ctx.bounds(), reveal);
        }
        state.revealed = self.reveal.is_some();

        let resting = self.radius.unwrap_or_else(|| {
            if self.surface {
                Corner::Panel.into()
            } else {
                CornerRadii::ZERO
            }
        });
        self.resize(state, ctx);
        let bounds = ctx.bounds();
        if self.resizing.is_some() || self.clips {
            ctx.set_children_clip(RoundedRect::new(
                bounds.inflate(-self.border_width()),
                resting,
            ));
        }
        let decorated = self.surface
            || self.background.is_some()
            || self.background_gradient.is_some()
            || self.border.is_some()
            || self.border_gradient.is_some()
            || !self.shadows.is_empty()
            || self.ground == Ground::Pane;
        if !decorated && !self.press.is_set() {
            return;
        }
        let radius = ctx.animate_curve("radius", resting, motion::MORPH);
        if decorated {
            let shape = RoundedRect::new(bounds, radius);
            for shadow in self.shadows.iter().rev() {
                ctx.draw_shadow(shape, *shadow);
            }
            let mut quad = Quad::new(bounds).radius(radius);
            quad = match self.background_gradient.as_deref() {
                Some(gradient) => quad.fill(*gradient),
                None => {
                    let background = self
                        .background
                        .or_else(|| self.surface.then_some(ctx.theme().palette().surface))
                        .unwrap_or(Color::TRANSPARENT);
                    quad.fill(ctx.animate_curve("background", background, motion::COLOR))
                }
            };
            if let Some((width, color)) = self.border {
                quad = quad.stroke(width, ctx.animate_curve("border", color, motion::COLOR));
            }
            if let Some((width, gradient)) = &self.border_gradient {
                quad = quad.stroke(*width, **gradient);
            }
            match self.ground {
                Ground::Pane => ctx.draw_pane(quad),
                Ground::Glass => {
                    ctx.draw_quad(quad);
                    ctx.glass(shape);
                }
                Ground::Plain => ctx.draw_quad(quad),
            }
        }
        if self.press.is_set() {
            state
                .ripple
                .paint(ctx, &state.press, RoundedRect::new(bounds, radius), true);
        }
    }

    fn event(&self, state: &mut ContainerState, event: &Event, ctx: &mut EventCtx<'_>) {
        match event {
            Event::PointerEntered => ctx.act(&self.on_hover, (true,)),
            Event::PointerExited => ctx.act(&self.on_hover, (false,)),
            _ => {}
        }
        if let Event::KeyPressed { key, modifiers, .. } = event {
            let pressed = key.to_lowercase();
            let answered = self
                .chords
                .iter()
                .find(|chord| chord.key == pressed && chord.modifiers == *modifiers);
            if let Some(chord) = answered {
                ctx.set_handled();
                ctx.call(&chord.handler, ());
                return;
            }
        }
        if let Event::Command(command) = event
            && let Some((_, handler)) = self
                .commands
                .iter()
                .find(|(candidate, _)| candidate == command)
        {
            ctx.set_handled();
            ctx.call(handler, ());
            return;
        }
        if self.modal && holds(event, ctx) {
            ctx.set_handled();
        }
        if !self.press.is_set() {
            return;
        }
        state.press.set_declines_focus(self.focusless);
        if state.press.update(event, true, ctx) {
            ctx.act(&self.press, ());
        }
    }

    fn behavior(&self, state: &Self::State) -> Behavior {
        Behavior::PAINTS_OFFSCREEN.when(self.reveal.is_some() && !state.revealed)
            | Behavior::MODAL.when(self.modal)
            | Behavior::TRACKS_HOVER.when(self.press.is_set())
            | Behavior::FOCUSABLE.when(self.press.is_set() && !self.focusless)
    }

    fn same(&self, other: &Self) -> bool {
        self.padding == other.padding
            && self.surface == other.surface
            && self.background == other.background
            && self.background_gradient == other.background_gradient
            && self.radius == other.radius
            && self.border == other.border
            && self.border_gradient == other.border_gradient
            && self.shadows == other.shadows
            && self.ground == other.ground
            && self.resizing == other.resizing
            && self.clips == other.clips
            && self.align_x == other.align_x
            && self.align_y == other.align_y
            && self.press == other.press
            && self.on_hover == other.on_hover
            && self.focusless == other.focusless
            && self.reveal == other.reveal
            && self.modal == other.modal
            && self.style == other.style
            && self
                .chords
                .iter()
                .map(|chord| (chord.key, chord.modifiers))
                .eq(other
                    .chords
                    .iter()
                    .map(|chord| (chord.key, chord.modifiers)))
            && self
                .commands
                .iter()
                .map(|(command, _)| *command)
                .eq(other.commands.iter().map(|(command, _)| *command))
    }
}

fn holds(event: &Event, ctx: &EventCtx<'_>) -> bool {
    match *event {
        Event::PointerPressed { position, .. }
        | Event::PointerReleased { position, .. }
        | Event::Scroll { position, .. } => ctx.bounds().contains(position),
        _ => false,
    }
}

impl std::fmt::Debug for Container {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Container").finish_non_exhaustive()
    }
}

impl View for Container {
    fn build(mut self) -> Element {
        let children = self.child.take().into_iter().collect();
        Element::with_children(self, children)
    }
}

impl Styled for Container {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}
