use paint_document::{Edit, LayerId, LayerKind, Properties};
use rikka_app::prelude::*;
use rikka_effects_forms::{form_sheet, labelled};
use rikka_psd::BlendMode;

use super::form::{self, Scale, Unit};
use super::{Context, Dialog, Kind, Outcome, tune};
use crate::paint::Paint;

const OPACITY: Scale = Scale::new(0.0..=100.0).unit(Unit::Percent);

#[derive(Debug)]
pub(crate) struct LayerProperties {
    layer: Option<LayerId>,
    group: bool,
    original_name: String,
    name: TextDocument,
    pub(crate) visible: bool,
    pub(crate) opacity: f32,
    pub(crate) blend: BlendMode,
}

impl LayerProperties {
    pub(crate) fn from_context(context: &Context<'_>) -> LayerProperties {
        let layer = context.layer;
        let original_name = layer.map(|layer| layer.name.clone()).unwrap_or_default();
        LayerProperties {
            layer: layer.map(|layer| layer.id),
            group: layer.is_some_and(|layer| layer.kind == LayerKind::Group),
            name: TextDocument::new(&original_name),
            original_name,
            visible: layer.is_none_or(|layer| layer.visible),
            opacity: layer.map_or(1.0, |layer| layer.opacity),
            blend: layer.map_or(BlendMode::Normal, |layer| layer.blend),
        }
    }

    fn name(&self) -> String {
        let typed = self.name.text().trim().to_owned();
        match typed.is_empty() {
            true => self.original_name.clone(),
            false => typed,
        }
    }
}

impl Dialog for LayerProperties {
    fn title(&self) -> Label {
        Kind::LayerProperties.name()
    }

    fn form(&self, _context: &Context<'_>) -> Element {
        let group = self.group;
        let at = form::blends(group)
            .iter()
            .position(|mode| *mode == self.blend)
            .unwrap_or(0);
        form_sheet(vec![
            labelled(
                tr!("paint.layers.name"),
                text_field(&self.name)
                    .on_submit(Paint::apply_session)
                    .fill_width(),
            ),
            form::toggle(
                tr!("paint.layers.visible"),
                self.visible,
                tune(|dialog: &mut LayerProperties, visible: bool| dialog.visible = visible),
            ),
            form::amount(
                tr!("paint.layers.opacity"),
                self.opacity * 100.0,
                &OPACITY,
                tune(|dialog: &mut LayerProperties, percent: f32| {
                    dialog.opacity = OPACITY.clamp(percent) / 100.0;
                }),
            ),
            labelled(
                tr!("paint.layers.blend"),
                form::blend(
                    form::blends(group)[at],
                    group,
                    tune(move |dialog: &mut LayerProperties, at: usize| {
                        dialog.blend = form::blends(group)[at];
                    }),
                ),
            ),
        ])
    }

    fn outcome(&self, _context: &Context<'_>) -> Option<Outcome> {
        Some(Outcome::Edit(Edit::SetProperties {
            layer: self.layer?,
            properties: Properties {
                name: self.name(),
                visible: self.visible,
                opacity: self.opacity,
                blend: self.blend,
            },
        }))
    }
}
