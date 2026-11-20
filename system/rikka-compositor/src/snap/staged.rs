use std::sync::Arc;
use std::time::Instant;

use rikka_compositor_desktop::{Target, WindowId};
use rikka_compositor_geometry::{Logical, Rect};
use rikka_compositor_types::OutputId;
use rikka_theme::Palette;

use super::element::Colours;
use super::paint;
use super::state::Snapping;
use crate::render::{Frosted, Moved, Staging, View};

impl Snapping {
    pub(crate) fn stage(
        &mut self,
        output: OutputId,
        screen: View,
        palette: Palette,
        now: Instant,
        staging: &mut Staging,
        geometry: impl Fn(WindowId) -> Option<Rect<i32, Logical>>,
    ) -> bool {
        let moving = self.advance(now);
        let colours = Colours::from_palette(palette);
        if let Some(drag) = self.drag().filter(|drag| drag.output() == output) {
            staging.beneath =
                paint::preview(drag, screen, colours).map(|panel| (drag.window(), panel));
            staging.top = paint::bar(drag, screen, colours);
        }
        if let Some(assist) = self.assist().filter(|assist| assist.output() == output) {
            staging.over = paint::offer(assist, screen, colours);
            staging.frosted = Some(Frosted {
                panes: Arc::clone(assist.frost()),
                alpha: assist.alpha(),
            });
            for card in assist.cards() {
                let (Some(shown), Some(geometry)) =
                    (assist.visual_for(card.window), geometry(card.window))
                else {
                    continue;
                };
                let target = Target::Window(card.window);
                staging.moved.insert(target, Moved { geometry, shown });
                staging.lifted.push(target);
            }
        }
        moving
    }
}
