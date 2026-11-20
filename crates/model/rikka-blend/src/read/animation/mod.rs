pub(crate) mod fcurve;
mod nla;

use rustc_hash::{FxHashMap, FxHashSet};

use super::modifier::Notes;
use crate::file::Record;
use fcurve::FCurve;

const KEYFRAME_STRIP: i64 = 0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Clock {
    pub(crate) start: i64,
    pub(crate) end: i64,
    pub(crate) current: i64,
    pub(crate) fps: f32,
}

impl Clock {
    pub(crate) fn from_scene(scene: Option<Record<'_>>) -> Self {
        let render = scene.and_then(|scene| scene.record("r"));
        let start = render.and_then(|render| render.int("sfra")).unwrap_or(1);
        let end = render
            .and_then(|render| render.int("efra"))
            .unwrap_or(250)
            .max(start);
        let rate = render
            .and_then(|render| render.int("frs_sec"))
            .unwrap_or(24) as f32;
        let base = render
            .and_then(|render| render.float("frs_sec_base"))
            .filter(|base| *base > 0.0)
            .unwrap_or(1.0);
        let fps = match rate / base {
            fps if fps.is_finite() && fps > 0.0 => fps,
            _ => 24.0,
        };
        let current = render
            .and_then(|render| render.int("cfra"))
            .unwrap_or(start);
        Self {
            start,
            end,
            current,
            fps,
        }
    }

    pub(crate) fn frame_count(&self) -> u64 {
        self.end.abs_diff(self.start).saturating_add(1)
    }

    pub(crate) fn time(&self, frame: f32) -> f32 {
        (frame - self.start as f32) / self.fps
    }
}

pub(crate) struct Breaks {
    from: f32,
    to: f32,
    frames: Vec<f64>,
    seen: FxHashSet<u32>,
}

impl Breaks {
    fn new(from: f32, to: f32) -> Self {
        let mut breaks = Self {
            from,
            to,
            frames: Vec::new(),
            seen: FxHashSet::default(),
        };
        breaks.push(from);
        breaks.push(to);
        breaks
    }

    fn capacity(&self) -> usize {
        (f64::from(self.to - self.from) as usize).saturating_add(2)
    }

    fn push(&mut self, frame: f32) -> bool {
        if self.frames.len() >= self.capacity() {
            return false;
        }
        if (self.from..=self.to).contains(&frame) && self.seen.insert(frame.to_bits()) {
            self.frames.push(f64::from(frame));
        }
        true
    }
}

#[derive(Default)]
pub(crate) struct Clip {
    curves: Vec<FCurve>,
    by_path: FxHashMap<String, Vec<(i64, usize)>>,
}

impl Clip {
    fn of_action(action: &Record<'_>, slot: i64, name: &str, notes: &mut Notes) -> Self {
        let records = match action.int("layer_array_num").unwrap_or(0) > 0 {
            true => layered(action, slot),
            false => action.list("curves"),
        };
        let mut clip = Clip::default();
        for record in records {
            if let Some(curve) = FCurve::read(&record) {
                if curve.unsupported_modifiers {
                    notes.unsupported(format!(
                        "some F-curve modifiers of \"{name}\" are not evaluated"
                    ));
                }
                let indices = clip.by_path.entry(curve.path.clone()).or_default();
                if !indices.iter().any(|&(index, _)| index == curve.index) {
                    indices.push((curve.index, clip.curves.len()));
                }
                clip.curves.push(curve);
            }
        }
        clip
    }

    pub(crate) fn find(&self, path: &str, index: i64) -> Option<&FCurve> {
        let &(_, slot) = self
            .by_path
            .get(path)?
            .iter()
            .find(|&&(slot_index, _)| slot_index == index)?;
        Some(&self.curves[slot])
    }
}

pub(crate) struct Curves {
    tracks: Vec<nla::Track>,
    action: Option<(Clip, Blending)>,
}

impl Curves {
    pub(crate) fn from_id(id: &Record<'_>, notes: &mut Notes) -> Option<Self> {
        let name = id.id_name();
        let animation = id.deref("adt")?;
        if !animation.list("drivers").is_empty() {
            notes.unsupported(format!("drivers of \"{name}\" not evaluated"));
        }
        let tracks = nla::tracks(&animation, &name, notes);
        let action = animation.id("action").map(|action| {
            let clip = Clip::of_action(
                &action,
                animation.int("slot_handle").unwrap_or(0),
                &name,
                notes,
            );
            let blending = match tracks.is_empty() {
                true => Blending::REPLACE,
                false => Blending {
                    mode: animation.int("act_blendmode").unwrap_or(0),
                    influence: animation
                        .float("act_influence")
                        .filter(|influence| *influence > 0.0)
                        .unwrap_or(1.0),
                },
            };
            (clip, blending)
        });
        let curves = Curves { tracks, action };
        let animated = curves.curves().next().is_some();
        animated.then_some(curves)
    }

    pub(crate) fn sample(&self, path: &str, index: i64, frame: f32, fallback: f32) -> f32 {
        let mut value = fallback;
        for track in &self.tracks {
            if let Some((clip, time, blending)) = track.at(frame)
                && let Some(curve) = clip.find(path, index)
            {
                value = blending.mix(value, curve.evaluate(time), path, index);
            }
        }
        if let Some((clip, blending)) = &self.action
            && let Some(curve) = clip.find(path, index)
        {
            value = blending.mix(value, curve.evaluate(frame), path, index);
        }
        value
    }

    pub(crate) fn breaks(&self, wanted: &dyn Fn(&str) -> bool, from: f32, to: f32) -> Vec<f64> {
        let mut breaks = Breaks::new(from, to);
        let open = self
            .tracks
            .iter()
            .all(|track| track.breaks(wanted, &mut breaks));
        if open && let Some((clip, _)) = &self.action {
            for curve in clip.curves.iter().filter(|curve| wanted(&curve.path)) {
                let mut room = true;
                curve.breaks(from, to, &mut |frame| {
                    room = breaks.push(frame);
                    room
                });
                if !room {
                    break;
                }
            }
        }
        breaks.frames
    }

    pub(crate) fn curves(&self) -> impl Iterator<Item = &FCurve> {
        self.tracks
            .iter()
            .flat_map(|track| track.clips())
            .chain(self.action.iter().map(|(clip, _)| clip))
            .flat_map(|clip| clip.curves.iter())
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Blending {
    pub(crate) mode: i64,
    pub(crate) influence: f32,
}

impl Blending {
    const REPLACE: Self = Self {
        mode: nla::REPLACE,
        influence: 1.0,
    };

    fn mix(self, lower: f32, value: f32, path: &str, index: i64) -> f32 {
        let influence = self.influence;
        match self.mode {
            nla::ADD => lower + influence * value,
            nla::SUBTRACT => lower - influence * value,
            nla::MULTIPLY => influence * (lower * value) + (1.0 - influence) * lower,
            nla::COMBINE => match path.ends_with("scale") {
                true => lower * value.powf(influence),
                false => {
                    let base = match path.ends_with("rotation_quaternion") && index == 0 {
                        true => 1.0,
                        false => 0.0,
                    };
                    lower + influence * (value - base)
                }
            },
            _ => lower * (1.0 - influence) + value * influence,
        }
    }
}

fn layered<'a>(action: &Record<'a>, slot: i64) -> Vec<Record<'a>> {
    let keyframe_data = action.pointers("strip_keyframe_data_array");
    let mut records = Vec::new();
    for layer in action
        .pointers("layer_array")
        .into_iter()
        .filter_map(|address| action.data_at(address))
    {
        for strip in layer
            .pointers("strip_array")
            .into_iter()
            .filter_map(|address| action.data_at(address))
        {
            if strip.int("strip_type").unwrap_or(KEYFRAME_STRIP) != KEYFRAME_STRIP {
                continue;
            }
            let Some(data) = strip
                .int("data_index")
                .and_then(|index| keyframe_data.get(usize::try_from(index).ok()?).copied())
                .and_then(|address| action.data_at(address))
            else {
                continue;
            };
            for bag in data
                .pointers("channelbag_array")
                .into_iter()
                .filter_map(|address| action.data_at(address))
            {
                if bag.int("slot_handle") != Some(slot) {
                    continue;
                }
                let count = bag.int("fcurve_array_num").unwrap_or(0).max(0) as usize;
                records.extend(
                    bag.pointers("fcurve_array")
                        .into_iter()
                        .take(count)
                        .filter_map(|address| action.data_at(address)),
                );
            }
        }
        if !records.is_empty() {
            break;
        }
    }
    records
}

pub(crate) fn quoted_name<'p>(path: &'p str, prefix: &str) -> Option<(String, &'p str)> {
    let rest = path.strip_prefix(prefix)?.strip_prefix("[\"")?;
    let mut name = String::new();
    let mut characters = rest.char_indices();
    while let Some((at, character)) = characters.next() {
        match character {
            '\\' => {
                if let Some((_, escaped)) = characters.next() {
                    name.push(escaped);
                }
            }
            '"' => {
                let tail = rest[at + 1..].strip_prefix(']')?;
                return Some((name, tail.strip_prefix('.').unwrap_or(tail)));
            }
            character => name.push(character),
        }
    }
    None
}
