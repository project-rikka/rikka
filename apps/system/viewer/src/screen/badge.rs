use rikka_app::prelude::{Behavior, Event, EventCtx, Key, PaintCtx, PointerButton, Widget};
use rikka_types::Color;
use rikka_ui::{IconGlyph, MaterialSymbol, palette};
use viewer_media::Step;

use crate::screen::Screen;
use crate::viewer::Viewer;

const SKIP: f32 = 5.0;
const VOLUME_STEP: f32 = 0.05;

const BADGE: f32 = 84.0;
const GLYPH: f32 = 40.0;
const RESTING_GLYPH: f32 = 96.0;
const BADGE_ALPHA: f32 = 0.55;
const RESTING_ALPHA: f32 = 0.35;

#[derive(Debug)]
pub(crate) struct Badge {
    pub(crate) resting: Option<MaterialSymbol>,
    pub(crate) playing: bool,
}

impl Widget for Badge {
    type State = ();

    fn paint(&self, (): &mut (), ctx: &mut PaintCtx<'_>) {
        let bounds = ctx.bounds();
        let palette = palette();

        if let Some(resting) = self.resting {
            ctx.draw_icon(
                bounds,
                IconGlyph::from(resting).spec(RESTING_GLYPH),
                palette.text_muted.with_alpha(RESTING_ALPHA),
            );
        }
        if self.playing {
            return;
        }

        ctx.draw_circle(bounds.center(), BADGE, Color::BLACK.with_alpha(BADGE_ALPHA));
        ctx.draw_icon(
            bounds,
            IconGlyph::from(MaterialSymbol::PLAY_ARROW).spec(GLYPH),
            Color::WHITE,
        );
    }

    fn event(&self, (): &mut (), event: &Event, ctx: &mut EventCtx<'_>) {
        match *event {
            Event::PointerPressed {
                button: PointerButton::Primary,
                ..
            } => {
                ctx.request_focus();
                ctx.run(|screen: &mut Screen| screen.play_pause());
            }
            Event::KeyPressed { key, .. } => {
                match key {
                    Key::Space | Key::Enter => ctx.run(|screen: &mut Screen| screen.play_pause()),
                    Key::ArrowLeft => ctx.run(|screen: &mut Screen| screen.skip(-SKIP)),
                    Key::ArrowRight => ctx.run(|screen: &mut Screen| screen.skip(SKIP)),
                    Key::ArrowUp => ctx.run(|screen: &mut Screen| screen.nudge_volume(VOLUME_STEP)),
                    Key::ArrowDown => {
                        ctx.run(|screen: &mut Screen| screen.nudge_volume(-VOLUME_STEP))
                    }
                    Key::PageUp => ctx.run(|viewer: &mut Viewer| viewer.step(Step::Previous)),
                    Key::PageDown => ctx.run(|viewer: &mut Viewer| viewer.step(Step::Next)),
                    _ => None,
                };
            }
            _ => {}
        }
    }

    fn behavior(&self, _state: &Self::State) -> Behavior {
        Behavior::FOCUSABLE | Behavior::AUTOFOCUS
    }
}
