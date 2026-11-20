use std::mem;

use rikka_image_edit::{Depth, Picture, Profile};
use rikka_psd::Encoded;

use crate::error::Result;
use crate::layer::{self, LayerId};
use crate::overlay::Overlays;
use crate::reductions::DirtyRegion;
use crate::region::Region;

const ENTRY_BYTES: u64 = 256;

const PATCH_SHARE_NUMERATOR: u64 = 3;

const PATCH_SHARE_DENOMINATOR: u64 = 4;

#[derive(Debug)]
pub(crate) struct State {
    pub(crate) content: Content,
    pub(crate) selection: Region,
    pub(crate) overlays: Overlays,
}

#[derive(Debug)]
pub(crate) enum Content {
    Picture {
        picture: Picture,
        visible: bool,
    },
    Photoshop {
        document: Box<rikka_psd::Document>,
        profile: Option<Profile>,
        touched: bool,
    },
}

pub(crate) fn stale_composite(document: &rikka_psd::Document, touched: bool) -> bool {
    touched && !document.layers.is_empty()
}

#[derive(Debug)]
pub(crate) enum Swap {
    Picture(Picture),
    Patch {
        at: [u32; 2],
        piece: Picture,
    },
    Shown {
        layer: LayerId,
        visible_before: bool,
    },
    Layer {
        layer: LayerId,
        record: Box<rikka_psd::Layer>,
    },
    Photoshop {
        document: Box<rikka_psd::Document>,
        profile: Option<Profile>,
        touched: bool,
    },
    Selection(Region),
    Content(Box<Content>),
    Overlays {
        previous_overlays: Overlays,
        bytes: u64,
    },
}

impl State {
    pub(crate) fn width(&self) -> u32 {
        match &self.content {
            Content::Picture { picture, .. } => picture.width(),
            Content::Photoshop { document, .. } => document.width,
        }
    }

    pub(crate) fn height(&self) -> u32 {
        match &self.content {
            Content::Picture { picture, .. } => picture.height(),
            Content::Photoshop { document, .. } => document.height,
        }
    }

    pub(crate) fn reselect(&mut self, region: Region) -> Option<Swap> {
        (self.selection != region)
            .then(|| Swap::Selection(mem::replace(&mut self.selection, region)))
    }

    pub(crate) fn overlay(&mut self, overlays: Overlays, replaced_bases: u64) -> Swap {
        let previous = mem::replace(&mut self.overlays, overlays);
        let bytes = previous.departed_bytes(&self.overlays) + replaced_bases;
        Swap::Overlays {
            previous_overlays: previous,
            bytes,
        }
    }
}

impl Swap {
    pub(crate) fn from_pictures(current: &mut Picture, next: Picture) -> Result<Option<Swap>> {
        if next.dimensions() != current.dimensions() {
            return Ok(Some(Swap::Picture(mem::replace(current, next))));
        }
        let changed = next.changed_area(current)?;
        let Some(area) = changed else {
            *current = next;
            return Ok(None);
        };
        let [left, top, width, height] = area;
        let whole = u64::from(current.width()) * u64::from(current.height());
        if u64::from(width) * u64::from(height) * PATCH_SHARE_DENOMINATOR
            >= whole * PATCH_SHARE_NUMERATOR
        {
            return Ok(Some(Swap::Picture(mem::replace(current, next))));
        }
        let piece = current.piece(area)?;
        *current = next;
        Ok(Some(Swap::Patch {
            at: [left, top],
            piece,
        }))
    }

    pub(crate) fn exchange(&mut self, state: &mut State) -> Result<()> {
        if let (Swap::Patch { at, piece }, Content::Picture { picture, .. }) =
            (&mut *self, &mut state.content)
        {
            let (width, height) = piece.dimensions();
            let current = picture.piece([at[0], at[1], width, height])?;
            *picture = picture.patched(piece, *at)?;
            *piece = current;
            return Ok(());
        }
        self.trade(state);
        Ok(())
    }

    fn trade(&mut self, state: &mut State) {
        match (self, &mut state.content) {
            (Swap::Selection(region), _) => mem::swap(region, &mut state.selection),
            (Swap::Content(content), _) => mem::swap(&mut **content, &mut state.content),
            (
                Swap::Overlays {
                    previous_overlays: overlays_before,
                    ..
                },
                _,
            ) => mem::swap(overlays_before, &mut state.overlays),
            (Swap::Picture(picture_before), Content::Picture { picture, .. }) => {
                mem::swap(picture_before, picture)
            }
            (Swap::Shown { visible_before, .. }, Content::Picture { visible, .. }) => {
                mem::swap(visible_before, visible);
            }
            (
                Swap::Shown {
                    layer,
                    visible_before,
                },
                Content::Photoshop {
                    document, touched, ..
                },
            ) => {
                if let Some(node) = layer::node_mut(document, *layer) {
                    let record = node.layer_mut();
                    let was = record.visible();
                    record.set_visible(*visible_before);
                    *visible_before = was;
                    *touched = true;
                }
            }
            (
                Swap::Layer { layer, record },
                Content::Photoshop {
                    document, touched, ..
                },
            ) => {
                if let Some(node) = layer::node_mut(document, *layer) {
                    mem::swap(node.layer_mut(), record);
                    *touched = true;
                }
            }
            (
                Swap::Photoshop {
                    document: previous_document,
                    profile: held_profile,
                    touched: held_touched,
                },
                Content::Photoshop {
                    document,
                    profile,
                    touched,
                },
            ) => {
                mem::swap(previous_document, document);
                mem::swap(held_profile, profile);
                mem::swap(held_touched, touched);
            }
            (swap, content) => unreachable!("{swap:?} cannot be exchanged with {content:?}"),
        }
    }

    pub(crate) fn touched(&self) -> DirtyRegion {
        match self {
            Swap::Selection(_) => DirtyRegion::Nothing,
            Swap::Patch { at, piece } => {
                DirtyRegion::Area([at[0], at[1], piece.width(), piece.height()])
            }
            _ => DirtyRegion::Everything,
        }
    }

    pub(crate) fn bytes(&self) -> u64 {
        ENTRY_BYTES
            + match self {
                Swap::Picture(picture) | Swap::Patch { piece: picture, .. } => {
                    picture_bytes(picture)
                }
                Swap::Shown { .. } => 0,
                Swap::Layer { record, .. } => layer_bytes(record),
                Swap::Photoshop { document, .. } => document_bytes(document),
                Swap::Selection(region) => region.held_bytes(),
                Swap::Content(content) => content_bytes(content),
                Swap::Overlays { bytes, .. } => *bytes,
            }
    }
}

pub(crate) fn picture_bytes(picture: &Picture) -> u64 {
    let per_pixel = match picture.depth() {
        Depth::Eight => 4,
        Depth::Sixteen => 8,
        Depth::Float => 16,
    };
    u64::from(picture.width()) * u64::from(picture.height()) * per_pixel
}

fn content_bytes(content: &Content) -> u64 {
    match content {
        Content::Picture { picture, .. } => picture_bytes(picture),
        Content::Photoshop { document, .. } => document_bytes(document),
    }
}

fn layer_bytes(record: &rikka_psd::Layer) -> u64 {
    record
        .channels
        .iter()
        .map(|channel| encoded_bytes(&channel.data))
        .sum()
}

fn document_bytes(document: &rikka_psd::Document) -> u64 {
    let composite: u64 = document.composite.planes.iter().map(encoded_bytes).sum();
    let layers: u64 = document.layer_records().into_iter().map(layer_bytes).sum();
    composite + layers
}

fn encoded_bytes(encoded: &Encoded) -> u64 {
    match encoded {
        Encoded::Raw(data) | Encoded::Zip(data) | Encoded::ZipPredicted(data) => data.len() as u64,
        Encoded::Rle { rows, packed } => (rows.len() * 4 + packed.len()) as u64,
    }
}
