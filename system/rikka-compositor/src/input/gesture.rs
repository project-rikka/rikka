use rikka_compositor_swipes::{Pointed, Scroll};
use rikka_compositor_types::Gesture;

use crate::context::{person, windows_ctx};
use crate::state::Compositor;
use crate::windows::overviewed;

impl Compositor {
    pub(super) fn gestured(&mut self, gesture: Gesture) {
        person!(self).activity();
        let swipes = self.session.swipes.clone();
        if swipes.is_none() && !self.windows.scrolling_mode() {
            return self.person().gesture(gesture);
        }
        let (windows, outputs, at) = (&self.windows, &self.outputs, self.cursor.position());
        let announcing = swipes.is_some();
        let outcome = self.input.swiping.handle(gesture, || {
            let published = outputs
                .containing(at)
                .map(|output| output.id())
                .filter(|output| outputs.wayland().is_published(*output));
            let sideways_kept = published.is_some_and(|output| overviewed(windows, output));
            Pointed {
                announcing,
                sideways_kept,
                scrolls_on: published
                    .filter(|output| !sideways_kept && windows.scrolls_on(*output)),
                output: published
                    .and_then(|output| outputs.wayland().name(output))
                    .map(str::to_owned),
            }
        });
        if let Some(swipes) = swipes {
            for swipe in outcome.announced {
                swipes.announce(swipe);
            }
        }
        for passed in outcome.passed {
            self.person().gesture(passed);
        }
        for scroll in outcome.scrolled {
            match scroll {
                Scroll::Began { output } => self.windows.begin_strip_swipe(output),
                Scroll::Moved { dx, time } => windows_ctx!(self).strip_swiped(dx, time),
                Scroll::Ended { time } => windows_ctx!(self).end_strip_swipe(time),
            }
        }
    }
}
