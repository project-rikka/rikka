mod animation_presets;
mod items;
mod markers;
mod paths;
mod tracks;
mod transitions;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rikka_effects::Effect;
use rikka_file_format::JsonFormat;
use serde::{Deserialize, Serialize};

use crate::clip::{Clip, ClipId, Source};
use crate::effect_slot::EffectSlot;
use crate::error::{Error, Result};
use crate::height::Height;
use crate::layer::Layer;
use crate::markers::{Marker, MarkerId};
use crate::media::{Media, MediaId, Picture, Sound};
use crate::project::{Format, Marks, Project};
use crate::rate::Rate;
use animation_presets::{
    StoredAnimationPresets, restored_animation_presets, stored_animation_presets,
};
use items::{
    StoredCamera, StoredGraphic, StoredRegion, StoredTreatment, restored_camera, restored_graphic,
    restored_region, restored_treatment, stored_camera, stored_graphic, stored_region,
    stored_treatment,
};
use markers::{StoredMarker, restored_markers, stored_markers};
use paths::{located, relative};
use tracks::{StoredKey, ordered, restored_scalar, stored_motion, stored_track};
use transitions::{StoredEnds, restored_ends, stored_ends};

pub const EXTENSION: &str = "rcut";

const FORMAT: JsonFormat = JsonFormat::new("rikka-cut");

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    rate: StoredRate,
    width: u32,
    height: u32,
    fitted: bool,
    media: Vec<StoredMedia>,
    layers: Vec<StoredLayer>,
    layer_height: f32,
    mark_in: Option<i64>,
    mark_out: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    markers: Vec<StoredMarker>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRate {
    numerator: u32,
    denominator: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredMedia {
    id: u64,
    path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    relative: Option<PathBuf>,
    duration_us: u64,
    picture: Option<StoredPicture>,
    sound: Option<StoredSound>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredPicture {
    width: u32,
    height: u32,
    rate: StoredRate,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSound {
    sample_rate: u32,
    channels: u16,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLayer {
    shown: bool,
    locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<f32>,
    clips: Vec<StoredClip>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredClip {
    id: u64,
    start: i64,
    length: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    media: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_in: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    treatment: Option<StoredTreatment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    graphic: Option<StoredGraphic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    region: Option<StoredRegion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    camera: Option<StoredCamera>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tracks: BTreeMap<String, Vec<StoredKey>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    effects: Vec<StoredSlot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    animation_presets: Option<StoredAnimationPresets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    transitions: Option<StoredEnds>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    markers: Vec<StoredMarker>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSlot {
    effect: Effect,
    enabled: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tracks: BTreeMap<String, Vec<StoredKey>>,
}

pub fn save(project: &Project, path: &Path) -> Result<()> {
    let folder = std::path::absolute(path)
        .map_err(|source| rikka_file_format::Error::Write {
            path: path.to_path_buf(),
            source,
        })?
        .parent()
        .map(Path::to_path_buf);
    Ok(FORMAT.save(path, &stored(project, folder.as_deref()))?)
}

pub(super) fn is_span(frames: i64) -> bool {
    (0..=Project::LONGEST).contains(&frames)
}

pub(crate) fn read(path: &Path) -> Result<Project> {
    let stored: Stored = FORMAT.load(path)?;
    let folder = path.parent().unwrap_or(Path::new("."));
    restored(stored, folder).map_err(|reason| Error::Damaged {
        path: path.to_path_buf(),
        reason,
    })
}

fn stored(project: &Project, folder: Option<&Path>) -> Stored {
    let rate = |rate: Rate| StoredRate {
        numerator: rate.numerator(),
        denominator: rate.denominator(),
    };
    Stored {
        rate: rate(project.format.rate),
        width: project.format.width,
        height: project.format.height,
        fitted: project.fitted,
        media: project
            .media
            .iter()
            .map(|(id, media)| StoredMedia {
                id: id.0,
                path: media.path().to_path_buf(),
                relative: folder.and_then(|folder| relative(media.path(), folder)),
                duration_us: media.duration().as_micros().min(u128::from(u64::MAX)) as u64,
                picture: media.picture().map(|picture| StoredPicture {
                    width: picture.width,
                    height: picture.height,
                    rate: rate(picture.rate),
                }),
                sound: media.sound().map(|sound| StoredSound {
                    sample_rate: sound.sample_rate,
                    channels: sound.channels,
                }),
            })
            .collect(),
        layers: project
            .layers
            .iter()
            .map(|layer| StoredLayer {
                shown: layer.visible,
                locked: layer.locked,
                height: layer.height.map(Height::px),
                clips: layer.clips.iter().map(stored_clip).collect(),
            })
            .collect(),
        layer_height: project.layer_height.px(),
        mark_in: project.marks.mark_in,
        mark_out: project.marks.mark_out,
        markers: stored_markers(&project.markers),
    }
}

fn restored(stored: Stored, folder: &Path) -> Result<Project, String> {
    let rate = |rate: &StoredRate| {
        Rate::new(rate.numerator, rate.denominator).map_err(|error| error.to_string())
    };
    let mut used_ids = BTreeSet::new();
    let mut claim = |id: u64| match id > 0 && used_ids.insert(id) {
        true => Ok(id),
        false => Err(format!("the id {id} is used twice")),
    };

    let mut media = BTreeMap::new();
    for stored_media in &stored.media {
        let path = located(&stored_media.path, stored_media.relative.as_deref(), folder);
        let mut restored = Media::new(path, Duration::from_micros(stored_media.duration_us));
        if let Some(picture) = &stored_media.picture {
            restored = restored.with_picture(Picture {
                width: picture.width,
                height: picture.height,
                rate: rate(&picture.rate)?,
            });
        }
        if let Some(sound) = &stored_media.sound {
            restored = restored.with_sound(Sound {
                sample_rate: sound.sample_rate,
                channels: sound.channels,
            });
        }
        media.insert(MediaId(claim(stored_media.id)?), restored);
    }

    let mut layers = Vec::with_capacity(stored.layers.len());
    let mut marked: Vec<(usize, ClipId, Vec<Marker>)> = Vec::new();
    for stored_layer in &stored.layers {
        let mut layer = Layer {
            visible: stored_layer.shown,
            locked: stored_layer.locked,
            height: stored_layer.height.map(Height::new),
            clips: Vec::with_capacity(stored_layer.clips.len()),
        };
        for clip in &stored_layer.clips {
            let restored = restored_clip(clip, &media)?;
            claim(clip.id)?;
            let markers = restored_markers(&clip.markers)
                .map_err(|reason| format!("clip {}: {reason}", clip.id))?;
            if !markers.is_empty() {
                marked.push((layers.len(), restored.id, markers));
            }
            layer.clips.push(restored);
        }
        layer.clips.sort_by_key(|clip| clip.start);
        if layer
            .clips
            .windows(2)
            .any(|pair| pair[0].end() > pair[1].start)
        {
            return Err("two clips on one layer overlap".to_owned());
        }
        layers.push(Arc::new(layer));
    }
    if !Format::SIDES.contains(&stored.width) || !Format::SIDES.contains(&stored.height) {
        return Err(format!(
            "the frame is {}×{}, outside {}–{} px",
            stored.width,
            stored.height,
            Format::SIDES.start(),
            Format::SIDES.end()
        ));
    }
    if [stored.mark_in, stored.mark_out]
        .into_iter()
        .flatten()
        .any(|mark| !is_span(mark))
    {
        return Err("a mark lies outside the project".to_owned());
    }

    let highest = used_ids.iter().copied().max().unwrap_or(0);
    let mut project = Project {
        format: Format {
            rate: rate(&stored.rate)?,
            width: stored.width,
            height: stored.height,
        },
        fitted: stored.fitted,
        media,
        layers,
        layer_height: Height::new(stored.layer_height),
        marks: Marks {
            mark_in: stored.mark_in,
            mark_out: stored.mark_out,
        },
        markers: Default::default(),
        next_id: highest + 1,
    };
    for marker in restored_markers(&stored.markers)? {
        let id = MarkerId(project.next_id());
        project.markers.put(id, marker);
    }
    for (layer, clip, markers) in marked {
        for marker in markers {
            let id = MarkerId(project.next_id());
            if let Some(target) = project.layer_mut(layer).find_mut(clip) {
                target.markers.put(id, marker);
            }
        }
    }
    project.normalize_transitions();
    Ok(project)
}

fn stored_clip(clip: &Clip) -> StoredClip {
    let mut stored = StoredClip {
        id: clip.id.0,
        start: clip.start,
        length: clip.length,
        media: None,
        source_in: None,
        treatment: None,
        graphic: None,
        region: None,
        camera: None,
        tracks: stored_motion(&clip.motion),
        effects: clip.effects.iter().map(stored_slot).collect(),
        animation_presets: stored_animation_presets(&clip.animation_presets),
        transitions: stored_ends(&clip.transitions),
        markers: stored_markers(&clip.markers),
    };
    match &clip.source {
        Source::Footage {
            media,
            source_in,
            treatment,
        } => {
            stored.media = Some(media.0);
            stored.source_in = Some(*source_in);
            stored.treatment = Some(stored_treatment(treatment));
        }
        Source::Graphic(graphic) => stored.graphic = Some(stored_graphic(graphic)),
        Source::Region(region) => stored.region = Some(stored_region(region)),
        Source::Camera(camera) => stored.camera = Some(stored_camera(camera)),
    }
    stored
}

fn restored_clip(clip: &StoredClip, media: &BTreeMap<MediaId, Media>) -> Result<Clip, String> {
    let stored_source = (
        &clip.media,
        clip.source_in,
        &clip.graphic,
        &clip.region,
        &clip.camera,
    );
    let source = match stored_source {
        (Some(named), source_in, None, None, None) => {
            if !media.contains_key(&MediaId(*named)) {
                return Err(format!("clip {} names missing media {named}", clip.id));
            }
            Source::Footage {
                media: MediaId(*named),
                source_in: source_in.unwrap_or(0),
                treatment: clip
                    .treatment
                    .as_ref()
                    .map(|treatment| restored_treatment(treatment, clip.id))
                    .transpose()?
                    .unwrap_or_default(),
            }
        }
        (None, None, Some(graphic), None, None) if clip.treatment.is_none() => {
            Source::Graphic(Arc::new(restored_graphic(graphic, clip.id)?))
        }
        (None, None, None, Some(region), None) if clip.treatment.is_none() => {
            Source::Region(Arc::new(restored_region(region, clip.id)?))
        }
        (None, None, None, None, Some(camera)) if clip.treatment.is_none() => {
            Source::Camera(Arc::new(restored_camera(camera, clip.id)?))
        }
        _ => {
            return Err(format!(
                "clip {} must hold one of media, a graphic, a region or a camera",
                clip.id
            ));
        }
    };
    if !is_span(clip.start)
        || clip.source_in.is_some_and(|at| !is_span(at))
        || clip.length < 1
        || !is_span(clip.start.saturating_add(clip.length))
    {
        return Err(format!("clip {} has no place in time", clip.id));
    }
    let mut restored = Clip::new(ClipId(clip.id), clip.start, clip.length, source);
    restored.motion =
        tracks::restored_motion(&clip.tracks, &restored, clip.id)?.cropped(&(0..clip.length));
    restored.effects = clip
        .effects
        .iter()
        .map(|slot| restored_slot(slot, clip))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(animation_presets) = &clip.animation_presets {
        if restored.camera().is_some() {
            return Err(format!(
                "clip {}: animation presets are not allowed here",
                clip.id
            ));
        }
        restored.animation_presets =
            restored_animation_presets(animation_presets, clip.id)?.checked();
    }
    if let Some(ends) = &clip.transitions {
        if restored.camera().is_some() {
            return Err(format!(
                "clip {}: transitions are not allowed here",
                clip.id
            ));
        }
        restored.transitions = restored_ends(ends, clip.id)?;
    }
    Ok(restored)
}

fn stored_slot(slot: &EffectSlot) -> StoredSlot {
    StoredSlot {
        effect: slot.effect.clone(),
        enabled: slot.enabled,
        tracks: slot
            .tracks
            .iter()
            .filter(|(_, track)| !track.is_empty())
            .map(|(knob, track)| (knob.clone(), stored_track(track)))
            .collect(),
    }
}

fn restored_slot(slot: &StoredSlot, clip: &StoredClip) -> Result<EffectSlot, String> {
    let tracks = slot
        .tracks
        .iter()
        .map(|(knob, keys)| {
            if !ordered(keys, clip.length) {
                return Err(format!("the keys of clip {} are out of order", clip.id));
            }
            let track =
                restored_scalar(keys).map_err(|reason| format!("clip {}: {reason}", clip.id))?;
            Ok((knob.clone(), track))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    Ok(EffectSlot {
        effect: slot.effect.clone(),
        enabled: slot.enabled,
        tracks,
    }
    .checked(clip.length))
}
