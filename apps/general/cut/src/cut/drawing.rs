use std::time::Duration;

use cut_project::{Appearance, Camera, Clip, ClipId, Edit, EffectSlot, Graphic, Redraw, Region};
use rikka_app::prelude::*;
use rikka_effects::Kind;
use rikka_figure::ShapeKind;

use super::Cut;
use crate::art;
use crate::live::LivePreview;
use crate::tool::Tool;
use crate::trimmer::Trimmer;

const ITEM_LENGTH: Duration = Duration::from_secs(5);

impl Cut {
    pub(crate) fn tool(&self) -> Tool {
        self.tool
    }

    pub(crate) fn live(&self) -> &LivePreview {
        &self.live
    }

    pub(crate) fn range_shape(&self) -> ShapeKind {
        self.range_shape
    }

    pub(crate) fn set_tool(&mut self, tool: Tool) {
        if self.tool == tool && self.session.popup.is_none() {
            skip_view();
            return;
        }
        self.session.popup = None;
        if let Tool::Range(kind) = tool {
            self.range_shape = kind;
        }
        self.tool = tool;
    }

    pub(crate) fn escape(&mut self) {
        if self.session.popup.take().is_some() {
            return;
        }
        if self.session.marking.is_some() {
            self.close_marker();
            return;
        }
        if self.trimmer != Trimmer::Select {
            self.trimmer = Trimmer::Select;
            return;
        }
        match self.tool {
            Tool::Select => self.select(Vec::new()),
            _ => self.tool = Tool::Select,
        }
    }

    pub(crate) fn select_everywhere(&mut self) {
        self.session.popup = None;
        self.tool = Tool::Select;
        self.trimmer = Trimmer::Select;
    }

    fn item_length(&self) -> i64 {
        self.project().rate().frames_in(ITEM_LENGTH).max(1)
    }

    fn insert(&mut self, graphic: Graphic, at: i64, layer: usize) -> Option<ClipId> {
        let typed = graphic.is_text();
        let length = self.item_length();
        self.edit(Edit::Insert {
            item: graphic.into(),
            at,
            length,
            layer,
        });
        let inserted = match self.session.selected.len() {
            1 => self.session.selected.iter().next().copied(),
            _ => None,
        };
        if typed && let Some(clip) = inserted {
            self.type_into(clip, None);
        }
        inserted
    }

    pub(crate) fn add(&mut self, tool: Tool) {
        let frame = self.transport.frame();
        let layer = self.project().front_layer(frame);
        self.add_at(tool, frame, layer);
    }

    pub(crate) fn add_at(&mut self, tool: Tool, frame: i64, layer: usize) {
        let placeholder = tr!("cut.art.text").to_string();
        let Some(graphic) = art::new_graphic(tool, self.project().format(), &placeholder) else {
            return;
        };
        self.insert(graphic, frame, layer);
    }

    fn insert_region(&mut self, region: Region, at: i64, layer: usize) {
        let length = self.item_length();
        let inserted = self.edit_selecting(Edit::Insert {
            item: region.into(),
            at,
            length,
            layer,
        });
        if let Some(&[clip]) = inserted.as_deref() {
            self.amend(Edit::Effects {
                clip,
                effects: vec![EffectSlot::new(Kind::GaussianBlur.default_effect())],
            });
        }
    }

    pub(crate) fn add_range(&mut self, shape: Option<ShapeKind>) {
        let frame = self.transport.frame();
        let layer = self.project().front_layer(frame);
        self.add_range_at(shape, frame, layer);
    }

    pub(crate) fn add_range_at(&mut self, shape: Option<ShapeKind>, frame: i64, layer: usize) {
        let region = art::range(shape, self.project().format());
        self.insert_region(region, frame, layer);
    }

    pub(crate) fn add_camera(&mut self) {
        let frame = self.transport.frame();
        let layer = self.project().front_layer(frame);
        self.add_camera_at(frame, layer);
    }

    pub(crate) fn add_camera_at(&mut self, frame: i64, layer: usize) {
        let length = self.item_length();
        self.edit(Edit::Insert {
            item: Camera::default().into(),
            at: frame,
            length,
            layer,
        });
    }

    pub(crate) fn draw(&mut self, tool: Tool, from: Point, to: Point) {
        let format = self.project().format();
        let frame = self.transport.frame();
        let layer = self.project().front_layer(frame);
        match tool {
            Tool::Range(kind) => {
                self.insert_region(art::ranged(kind, format, from, to), frame, layer)
            }
            _ => {
                let Some(graphic) = art::drawn(tool, format, from, to) else {
                    return;
                };
                self.insert(graphic, frame, layer);
            }
        }
        self.tool = Tool::Select;
    }

    pub(crate) fn write_at(&mut self, at: Point) {
        let placeholder = tr!("cut.art.text").to_string();
        let graphic = art::new_text(self.project().format(), at, &placeholder);
        let frame = self.transport.frame();
        let layer = self.project().front_layer(frame);
        self.insert(graphic, frame, layer);
        self.tool = Tool::Select;
    }

    pub(crate) fn redraw(&mut self, changes: Vec<(ClipId, Appearance)>, by: Redraw) {
        self.live.settle();
        if changes.is_empty() {
            skip_view();
            return;
        }
        let at = self.transport.frame();
        self.edit(Edit::Redraw { changes, by, at });
    }

    pub(crate) fn selected_clip(&self) -> Option<&Clip> {
        let [id] = self.session.selected.iter().copied().collect::<Vec<_>>()[..] else {
            return None;
        };
        self.project().locate(id).map(|(_, clip)| clip)
    }

    pub(crate) fn selected_graphic(&self) -> Option<(ClipId, Graphic)> {
        let clip = self.selected_clip()?;
        let frame = self.transport.frame();
        Some((
            clip.id(),
            Graphic {
                placement: clip.placement_at(frame),
                art: clip.art_at(frame)?,
            },
        ))
    }
}
