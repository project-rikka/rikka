use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Range, RangeInclusive};
use std::path::Path;
use std::sync::Arc;

use rikka_types::{Point, Size};

use crate::clip::{Clip, ClipId, Source};
use crate::footage::Treatment;
use crate::graphic::Graphic;
use crate::height::Height;
use crate::layer::Layer;
use crate::markers::Markers;
use crate::media::{Media, MediaId, Stream};
use crate::property::Property;
use crate::rate::{Rate, gcd};
use crate::region::Region;
use crate::visual::Visual;

const STARTING_LAYERS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    pub rate: Rate,
    pub width: u32,
    pub height: u32,
}

impl Format {
    #[must_use]
    pub fn center(&self) -> Point {
        Point::new(self.width as f32 / 2.0, self.height as f32 / 2.0)
    }

    #[must_use]
    pub fn aspect(&self) -> (u32, u32) {
        let common = gcd(self.width, self.height).max(1);
        (self.width / common, self.height / common)
    }

    #[must_use]
    pub fn fitted(&self, width: u32, height: u32) -> Size {
        if width == 0 || height == 0 {
            return Size::new(self.width as f32, self.height as f32);
        }
        let (width, height) = (width as f32, height as f32);
        let scale = (self.width as f32 / width).min(self.height as f32 / height);
        Size::new(width * scale, height * scale)
    }
}

impl Default for Format {
    fn default() -> Format {
        Format {
            rate: Rate::VIDEO,
            width: 1920,
            height: 1080,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub mark_in: Option<i64>,
    pub mark_out: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Edge {
    Head,
    Tail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveLimits {
    pub earliest: i64,
    pub highest: i32,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub(crate) format: Format,
    pub(crate) fitted: bool,
    pub(crate) media: BTreeMap<MediaId, Media>,
    pub(crate) layers: Vec<Arc<Layer>>,
    pub(crate) layer_height: Height,
    pub(crate) marks: Marks,
    pub(crate) markers: Markers,
    pub(crate) next_id: u64,
}

impl PartialEq for Project {
    fn eq(&self, other: &Project) -> bool {
        self.format == other.format
            && self.fitted == other.fitted
            && self.next_id == other.next_id
            && self.marks == other.marks
            && self.markers == other.markers
            && self.layer_height == other.layer_height
            && self.layers.len() == other.layers.len()
            && self
                .layers
                .iter()
                .zip(&other.layers)
                .all(|(one, another)| Arc::ptr_eq(one, another) || one == another)
            && self.media == other.media
    }
}

impl Default for Project {
    fn default() -> Project {
        Project::new()
    }
}

impl Project {
    pub const LONGEST: i64 = 1 << 40;

    #[must_use]
    pub fn new() -> Project {
        Project {
            format: Format::default(),
            fitted: false,
            media: BTreeMap::new(),
            layers: vec![Arc::default(); STARTING_LAYERS],
            layer_height: Height::default(),
            marks: Marks::default(),
            markers: Markers::default(),
            next_id: 1,
        }
    }

    #[must_use]
    pub fn format(&self) -> Format {
        self.format
    }

    #[must_use]
    pub fn rate(&self) -> Rate {
        self.format.rate
    }

    #[must_use]
    pub fn marks(&self) -> Marks {
        self.marks
    }

    #[must_use]
    pub fn media(&self, id: MediaId) -> Option<&Media> {
        self.media.get(&id)
    }

    #[must_use]
    pub fn media_frames(&self, id: MediaId) -> i64 {
        self.media(id)
            .map_or(0, |media| media.frames(self.format.rate))
    }

    fn carries(&self, clip: &Clip, stream: Stream) -> bool {
        clip.media()
            .and_then(|media| self.media(media))
            .is_some_and(|media| media.has(stream))
    }

    #[must_use]
    pub fn has_sound(&self, clip: &Clip) -> bool {
        self.carries(clip, Stream::Sound)
    }

    #[must_use]
    pub fn is_audible(&self) -> bool {
        self.clips().any(|(_, clip)| self.has_sound(clip))
    }

    #[must_use]
    pub fn layers(&self) -> &[Arc<Layer>] {
        &self.layers
    }

    #[must_use]
    pub fn layer(&self, index: usize) -> Option<&Layer> {
        self.layers.get(index).map(Arc::as_ref)
    }

    #[must_use]
    pub fn layer_height(&self) -> Height {
        self.layer_height
    }

    #[must_use]
    pub fn height_of(&self, layer: usize) -> Height {
        self.layer(layer)
            .and_then(Layer::height)
            .unwrap_or(self.layer_height)
    }

    #[must_use]
    pub fn locate(&self, id: ClipId) -> Option<(usize, &Clip)> {
        self.layers
            .iter()
            .enumerate()
            .find_map(|(index, layer)| layer.find(id).map(|clip| (index, clip)))
    }

    pub fn clips(&self) -> impl Iterator<Item = (usize, &Clip)> {
        self.layers
            .iter()
            .enumerate()
            .flat_map(|(index, layer)| layer.clips.iter().map(move |clip| (index, clip)))
    }

    #[must_use]
    pub fn end(&self) -> i64 {
        self.layers
            .iter()
            .map(|layer| layer.end())
            .max()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.iter().all(|layer| layer.clips.is_empty())
    }

    pub fn visuals_at(&self, frame: i64) -> impl Iterator<Item = (usize, &Clip, Visual)> {
        self.shown_at(frame).filter_map(move |(index, clip)| {
            self.visual(clip, frame).map(|visual| (index, clip, visual))
        })
    }

    #[must_use]
    pub fn visual(&self, clip: &Clip, frame: i64) -> Option<Visual> {
        let placement = clip.placement_at(frame);
        match &clip.source {
            Source::Graphic(_) => Some(Visual::Graphic(Graphic {
                placement,
                art: clip.art_at(frame)?,
            })),
            Source::Camera(_) => None,
            Source::Region(region) => Some(Visual::Region(Region {
                placement,
                ..**region
            })),
            Source::Footage {
                media, treatment, ..
            } => {
                let picture = self.media(*media)?.picture()?;
                Some(Visual::Footage {
                    treatment: Treatment {
                        placement,
                        ..*treatment
                    },
                    picture: self.format.fitted(picture.width, picture.height),
                })
            }
        }
    }

    #[must_use]
    pub fn is_visual(&self, clip: &Clip) -> bool {
        match &clip.source {
            Source::Footage { .. } => self.carries(clip, Stream::Picture),
            Source::Graphic(_) | Source::Region(_) => true,
            Source::Camera(_) => false,
        }
    }

    #[must_use]
    pub fn animates(&self, clip: &Clip) -> bool {
        self.is_visual(clip) || clip.camera().is_some() || self.has_sound(clip)
    }

    #[must_use]
    pub fn keyable(&self, clip: &Clip, property: &Property) -> bool {
        let fits = match property {
            Property::Mix(_) => self.has_sound(clip),
            Property::Camera(_) => clip.camera().is_some(),
            Property::Placement(_) | Property::Art(_) | Property::Effect { .. } => {
                self.is_visual(clip)
            }
        };
        fits && clip.accepts(property)
    }

    fn shown_at(&self, frame: i64) -> impl Iterator<Item = (usize, &Clip)> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.visible)
            .filter_map(move |(index, layer)| layer.clip_at(frame).map(|clip| (index, clip)))
    }

    #[must_use]
    pub fn front_layer(&self, frame: i64) -> usize {
        self.layers
            .iter()
            .enumerate()
            .rev()
            .find(|(_, layer)| layer.visible && layer.clip_at(frame).is_some())
            .map_or(0, |(index, _)| index + 1)
    }

    #[must_use]
    pub fn clips_under(&self, frame: i64) -> Vec<ClipId> {
        self.layers
            .iter()
            .filter(|layer| !layer.locked)
            .filter_map(|layer| layer.clip_at(frame))
            .map(Clip::id)
            .collect()
    }

    #[must_use]
    pub fn edit_frames(&self) -> Vec<i64> {
        self.points(&[])
    }

    #[must_use]
    pub fn edit_frames_except(&self, moving: &[ClipId]) -> Vec<i64> {
        self.points(moving)
    }

    #[must_use]
    pub fn next_change(&self, after: i64, stream: Stream) -> Option<i64> {
        let edges = self
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .filter_map(|layer| {
                let first = layer.clips.partition_point(|clip| clip.end() <= after);
                layer.clips[first..]
                    .iter()
                    .find(|clip| self.carries(clip, stream))
                    .map(|clip| {
                        if clip.start > after {
                            clip.start
                        } else {
                            clip.end()
                        }
                    })
            });
        let passing = (0..self.layers.len())
            .filter(|index| self.layers[*index].visible)
            .flat_map(|index| {
                self.transition_spans(index)
                    .into_iter()
                    .flat_map(|transition_span| {
                        [transition_span.span.start, transition_span.span.end]
                    })
                    .find(|frame| *frame > after)
            });
        edges.chain(passing).min()
    }

    fn points(&self, skipped: &[ClipId]) -> Vec<i64> {
        let mut points = BTreeSet::new();
        for layer in self.layers.iter().filter(|layer| layer.visible) {
            for clip in layer
                .clips
                .iter()
                .filter(|clip| !skipped.contains(&clip.id))
            {
                points.insert(clip.start);
                points.insert(clip.end());
            }
        }
        points.into_iter().collect()
    }

    #[must_use]
    pub fn editable(&self, clips: &[ClipId]) -> Vec<ClipId> {
        let requested: BTreeSet<ClipId> = clips.iter().copied().collect();
        self.layers
            .iter()
            .filter(|layer| !layer.locked)
            .flat_map(|layer| &layer.clips)
            .map(|clip| clip.id)
            .filter(|id| requested.contains(id))
            .collect()
    }

    #[must_use]
    pub(crate) fn free_layer(&self, from: usize, span: &Range<i64>) -> usize {
        (from..)
            .find(|index| self.layer(*index).is_none_or(|layer| layer.is_free(span)))
            .unwrap_or(from)
    }

    #[must_use]
    pub fn move_limits(&self, clips: &[ClipId]) -> MoveLimits {
        let mut earliest = i64::MIN;
        let mut highest = i32::MAX;
        for (index, clip) in clips.iter().filter_map(|id| self.locate(*id)) {
            earliest = earliest.max(-clip.start);
            highest = highest.min(index as i32);
        }
        match earliest == i64::MIN {
            true => MoveLimits {
                earliest: 0,
                highest: 0,
            },
            false => MoveLimits { earliest, highest },
        }
    }

    #[must_use]
    pub fn trim_limits(&self, clips: &[ClipId], edge: Edge) -> RangeInclusive<i64> {
        let mut least = i64::MIN;
        let mut most = i64::MAX;
        let mut any = false;
        for (index, clip) in clips.iter().filter_map(|id| self.locate(*id)) {
            any = true;
            let layer = &self.layers[index];
            match edge {
                Edge::Head => {
                    if let Source::Footage { source_in, .. } = &clip.source {
                        least = least.max(-((*source_in as f64 / clip.speed()).floor() as i64));
                    }
                    least = least.max(layer.previous_end(clip.id) - clip.start);
                    most = most.min(clip.length - 1);
                }
                Edge::Tail => {
                    least = least.max(1 - clip.length);
                    if let Source::Footage {
                        media, source_in, ..
                    } = &clip.source
                    {
                        let available =
                            (self.media_frames(*media) - source_in) as f64 / clip.speed();
                        most = most.min(available.floor() as i64 - clip.length);
                    }
                    if let Some(next) = layer.next_start(clip.id) {
                        most = most.min(next - clip.end());
                    }
                }
            }
        }
        match any && least <= most {
            true => least..=most,
            false => 0..=0,
        }
    }

    #[must_use]
    pub fn slip_limits(&self, clips: &[ClipId]) -> RangeInclusive<i64> {
        let mut least = i64::MIN;
        let mut most = i64::MAX;
        let mut any = false;
        for (_, clip) in clips.iter().filter_map(|id| self.locate(*id)) {
            any = true;
            let Source::Footage {
                media, source_in, ..
            } = &clip.source
            else {
                return 0..=0;
            };
            least = least.max(-source_in);
            most = most.min(self.media_frames(*media) - source_in - clip.source_span());
        }
        match any && least <= most {
            true => least..=most,
            false => 0..=0,
        }
    }

    pub(crate) fn media_at(&self, path: &Path) -> Option<MediaId> {
        self.media
            .iter()
            .find(|(_, media)| media.path() == path)
            .map(|(id, _)| *id)
    }

    pub(crate) fn adopt(&mut self, media: &Media) -> MediaId {
        if let Some(id) = self.media_at(media.path()) {
            return id;
        }
        let id = MediaId(self.next_id());
        self.media.insert(id, media.clone());
        id
    }

    pub(crate) fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub(crate) fn next_clip_id(&mut self) -> ClipId {
        ClipId(self.next_id())
    }

    pub(crate) fn layer_mut(&mut self, index: usize) -> &mut Layer {
        if index >= self.layers.len() {
            self.layers.resize(index + 1, Arc::default());
        }
        Arc::make_mut(&mut self.layers[index])
    }
}
