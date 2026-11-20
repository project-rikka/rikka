use rikka_compositor_desktop::{Visibility, WindowId};
use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_types::OutputId;

use super::Windows;
use crate::context::WindowsCtx;

#[derive(Debug)]
pub(super) struct OutputAnchor {
    window: WindowId,
    output: OutputId,
    offset: Point<i32, Logical>,
}

fn fit_within(usable: Rect<i32, Logical>, size: Size<i32, Logical>) -> Rect<i32, Logical> {
    Rect::new(
        usable.loc,
        Size::new(
            (usable.size.w - size.w).max(0),
            (usable.size.h - size.h).max(0),
        ),
    )
}

impl Windows {
    pub(super) fn output_anchors(&self) -> Vec<OutputAnchor> {
        let desktop = &self.desktop;
        desktop
            .stacking()
            .filter_map(|window| {
                let record = desktop.window(window)?;
                let policy = desktop.policy(window)?;
                let free = record.visibility() == Visibility::Shown
                    && !self.is_tiled(window)
                    && policy.mode.is_free();
                if !free {
                    return None;
                }
                let geometry = record.geometry();
                let (output, area) = desktop
                    .outputs()
                    .filter_map(|(output, output_record)| {
                        let overlap = output_record.geometry().intersection(geometry)?;
                        let covered = i64::from(overlap.size.w) * i64::from(overlap.size.h);
                        Some((covered, output, output_record.geometry()))
                    })
                    .max_by_key(|(covered, _, _)| *covered)
                    .map(|(_, output, area)| (output, area))?;
                Some(OutputAnchor {
                    window,
                    output,
                    offset: geometry.loc - area.loc,
                })
            })
            .collect()
    }
}

impl WindowsCtx<'_> {
    pub(super) fn restore_anchors(&mut self, anchors: Vec<OutputAnchor>) {
        for anchor in anchors {
            let Some(record) = self.windows.desktop.window(anchor.window) else {
                continue;
            };
            let geometry = record.geometry();
            let output = match self.windows.desktop.output(anchor.output) {
                Some(_) => anchor.output,
                None => match self.windows.output_of(anchor.window) {
                    Some(output) => output,
                    None => continue,
                },
            };
            let Some(area) = self
                .windows
                .desktop
                .output(output)
                .map(|output_record| output_record.geometry())
            else {
                continue;
            };
            let room = fit_within(self.windows.usable_area(output), geometry.size);
            let at = (area.loc + anchor.offset).constrain(room);
            if at != geometry.loc {
                self.move_window(anchor.window, at);
            }
        }
    }
}
