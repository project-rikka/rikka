use rikka_compositor_geometry::{Physical, Size};
use rikka_compositor_output::{Mode, OutputInfo};
use rikka_picture::RefreshRate;

use super::state::State;

const FALLBACK: (i32, i32) = (1920, 1080);

pub(crate) fn display_output(size: Option<Size<i32, Physical>>, scale: f64) -> OutputInfo {
    let refresh = RefreshRate::published().unwrap_or(RefreshRate::SIXTY_HERTZ);
    OutputInfo {
        description: "Rikka nested view".into(),
        make: "Rikka".into(),
        model: "Nested".into(),
        mode: Mode {
            size: size.unwrap_or_else(|| Size::new(FALLBACK.0, FALLBACK.1)),
            refresh_millihertz: i32::try_from(refresh.millihertz()).unwrap_or(i32::MAX),
        },
        scale,
        ..OutputInfo::default()
    }
}

impl State {
    pub(crate) fn describe_output(&mut self) {
        let mut largest: Option<Size<i32, Physical>> = None;
        let mut scale = 1.0;
        for record in self.views.records.values() {
            let Some(size) = record.desired.size else {
                continue;
            };
            scale = record.desired.scale;
            let physical = Size::<i32, Physical>::new(
                (f64::from(size.w) * record.desired.scale).round() as i32,
                (f64::from(size.h) * record.desired.scale).round() as i32,
            );
            largest = Some(match largest {
                Some(known) => Size::new(known.w.max(physical.w), known.h.max(physical.h)),
                None => physical,
            });
        }
        let info = display_output(largest, scale);
        self.outputs.publish(&self.display, self.output, info);
    }
}
