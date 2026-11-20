use animate_body::{Panel, Side};
use animate_cast::frame_of;
use animate_fluid::{EmitterSection, EmitterSetting};
use animate_look::{GraphRef, LookGroup};
use animate_motion::Part;
use rikka_i18n::{Label, tr};

use crate::dope::Names;

pub(crate) fn clock(time: f32) -> String {
    animate_ui::frame_clock(frame_of(time))
}

pub(crate) fn seconds(seconds: f32) -> Label {
    tr!(
        "animate.liquid.seconds",
        seconds = format!("{seconds:.2}").as_str()
    )
}

pub(crate) fn pin_lane(at: usize) -> Label {
    tr!("animate.hold.pin_lane", number = at as u32 + 1)
}

pub(crate) fn look_group(group: &LookGroup, title: Option<&str>) -> Label {
    match group {
        LookGroup::Node { graph, node, .. } => {
            let node = title.map_or_else(|| node.get().to_string(), str::to_owned);
            let node = node.as_str();
            match graph {
                GraphRef::World => tr!("animate.look.track.world", node = node),
                GraphRef::Composite => tr!("animate.look.track.composite", node = node),
                GraphRef::Material(material) => tr!(
                    "animate.look.track.material",
                    material = material.as_str(),
                    node = node
                ),
            }
        }
        LookGroup::Slot { figure, material } => tr!(
            "animate.look.track.slot",
            figure = figure.as_str(),
            number = material + 1
        ),
    }
}

impl Names {
    pub fn new(sound: Option<String>) -> Names {
        Names {
            switches: tr!("animate.track.switches"),
            unlisted: tr!("animate.track.unlisted"),
            unmatched: tr!("animate.track.unmatched"),
            camera: tr!("animate.track.camera"),
            parts: tr!("animate.track.parts"),
            morphs: Panel::ALL.map(morph_group),
            part: Part::ALL.into_iter().map(animate_ui::part_label).collect(),
            sections: EmitterSection::ALL.into_iter().map(section).collect(),
            settings: EmitterSetting::ALL
                .iter()
                .copied()
                .map(animate_ui::emitter_setting_label)
                .collect(),
            pour: tr!("animate.track.pour"),
            wind: tr!("animate.track.wind"),
            winds: animate_wind::WindSetting::ALL
                .iter()
                .copied()
                .map(animate_ui::wind_setting_label)
                .collect(),
            hands: Side::BOTH.map(animate_ui::holding_hand_label),
            sound,
        }
    }
}

fn morph_group(panel: Panel) -> Label {
    match panel {
        Panel::Eyebrow => tr!("animate.track.morph_eyebrow"),
        Panel::Eye => tr!("animate.track.morph_eye"),
        Panel::Mouth => tr!("animate.track.morph_mouth"),
        Panel::Other => tr!("animate.track.morph_other"),
    }
}

fn section(section: EmitterSection) -> Label {
    tr!(&format!("animate.liquid.section.{}", section.id()))
}
