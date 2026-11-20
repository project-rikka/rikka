use std::sync::Arc;

use rikka_text::TextLayout;
use rikka_types::Size;
use smallvec::SmallVec;

use crate::paint::MeasureSpec;
use crate::text::shaping::ShapeRequest;
use crate::text::{TextRun, elide};

const KEPT_WIDTHS: usize = 8;

const INLINE_WIDTHS: usize = 4;

type Widened<T> = SmallVec<[T; INLINE_WIDTHS]>;

#[derive(Debug)]
pub(crate) struct Leaf {
    pub(crate) spec: MeasureSpec,
    sizes: Widened<Measured>,
    requested_widths: Widened<Option<u32>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Widths {
    pub(super) least: f32,
    pub(super) most: f32,
    pub(super) natural: Size,
}

impl Leaf {
    pub(crate) fn new(spec: MeasureSpec) -> Self {
        Self {
            spec,
            sizes: SmallVec::new(),
            requested_widths: SmallVec::new(),
        }
    }

    pub(crate) fn respec(&mut self, spec: MeasureSpec) {
        self.spec = spec;
        self.sizes.clear();
    }

    fn foreseen(&self) -> impl Iterator<Item = Option<f32>> + '_ {
        let first_sweep = match (&self.spec, self.requested_widths.is_empty()) {
            (MeasureSpec::Text { .. }, true) => Some(None),
            _ => None,
        };
        self.requested_widths
            .iter()
            .map(|width| width.map(f32::from_bits))
            .chain(first_sweep)
    }

    pub(crate) fn prime(&mut self) -> Option<ShapeRequest> {
        if !matches!(self.spec, MeasureSpec::Text { .. }) {
            return None;
        }
        let foreseen: SmallVec<[Option<f32>; KEPT_WIDTHS]> = self.foreseen().collect();
        let MeasureSpec::Text { run, inset, .. } = &self.spec else {
            return None;
        };
        let mut missing: SmallVec<[Option<f32>; 2]> = SmallVec::new();
        for width in foreseen {
            let key = width.map(f32::to_bits);
            if self.sizes.iter().any(|measured| measured.key == key) {
                continue;
            }
            let limit = crate::paint::limit(run, width);
            let laid = match run.content.is_empty() {
                true => None,
                false => match rikka_text::recent_layout(run.as_shape(), limit) {
                    Some(laid) => Some(laid),
                    None => {
                        if !missing.contains(&limit) {
                            missing.push(limit);
                        }
                        continue;
                    }
                },
            };
            let size = measured_size(run, *inset, laid.as_deref());
            remember(&mut self.sizes, Measured { key, size, laid });
        }
        (!missing.is_empty()).then(|| ShapeRequest {
            run: run.clone(),
            widths: missing,
        })
    }

    pub(super) fn widths(&mut self) -> Widths {
        crate::stats::measure_called();
        match &self.spec {
            MeasureSpec::Fixed(size) => Widths {
                least: size.width,
                most: size.width,
                natural: *size,
            },
            MeasureSpec::Shrinkable { least, natural } => Widths {
                least: *least,
                most: natural.width,
                natural: *natural,
            },
            MeasureSpec::Text { run, inset, least } => {
                let inset = *inset;
                let given = match least {
                    Some(least) => Some(least.width),
                    None => elide::least_width(run).map(|width| width + inset.width),
                };
                let (natural, laid) = self.measure(None);
                let least = given.unwrap_or_else(|| {
                    let content = laid.map_or(0.0, |laid| laid.min_content_width().ceil());
                    content + inset.width
                });
                Widths {
                    least,
                    most: natural.width,
                    natural,
                }
            }
            MeasureSpec::Document(_) => {
                let natural = self.sized(None);
                Widths {
                    least: 0.0,
                    most: natural.width,
                    natural,
                }
            }
        }
    }

    pub(super) fn size_for_width(&mut self, width: f32) -> Size {
        crate::stats::measure_called();
        let inset = match &self.spec {
            MeasureSpec::Fixed(size) => return *size,
            MeasureSpec::Shrinkable { least, natural } => {
                return Size::new(width.min(natural.width).max(*least), natural.height);
            }
            MeasureSpec::Text { inset, .. } => *inset,
            MeasureSpec::Document(document) => document.inset(),
        };
        self.sized(Some((width - inset.width).max(0.0)))
    }

    pub(super) fn wraps(&self) -> bool {
        match &self.spec {
            MeasureSpec::Fixed(_) | MeasureSpec::Shrinkable { .. } => false,
            MeasureSpec::Text { run, .. } => !elide::elides(run),
            MeasureSpec::Document(document) => document.wraps(),
        }
    }

    pub(super) fn least_height(&self) -> Option<f32> {
        match &self.spec {
            MeasureSpec::Text {
                least: Some(least), ..
            } => Some(least.height),
            MeasureSpec::Document(document) => Some(document.least()),
            _ => None,
        }
    }

    fn sized(&mut self, limit: Option<f32>) -> Size {
        self.measure(limit).0
    }

    fn measure(&mut self, limit: Option<f32>) -> (Size, Option<Arc<TextLayout>>) {
        let (run, inset) = match &self.spec {
            MeasureSpec::Fixed(size) => return (*size, None),
            MeasureSpec::Shrinkable { natural, .. } => return (*natural, None),
            MeasureSpec::Document(document) => return (document.size_at(limit), None),
            MeasureSpec::Text { run, inset, .. } => (run, *inset),
        };
        let key = limit.map(f32::to_bits);
        if let Some(hit) = self.sizes.iter().find(|measured| measured.key == key) {
            return (hit.size, hit.laid.clone());
        }
        let limit = crate::paint::limit(run, limit);
        let served = self.sizes.iter().find_map(|measured| {
            let laid = measured.laid.as_ref()?;
            let serves = match limit {
                Some(width) => laid.serves_width(width),
                None => laid.width().is_none(),
            };
            serves.then(|| (measured.size, laid.clone()))
        });
        let (size, laid) = match served {
            Some((size, laid)) => {
                self.requested_widths.retain(|width| *width != key);
                (size, Some(laid))
            }
            None => {
                let laid =
                    (!run.content.is_empty()).then(|| rikka_text::layout(run.as_shape(), limit));
                note(&mut self.requested_widths, key);
                (measured_size(run, inset, laid.as_deref()), laid)
            }
        };
        remember(
            &mut self.sizes,
            Measured {
                key,
                size,
                laid: laid.clone(),
            },
        );
        (size, laid)
    }

    pub(crate) fn laid(&self, run: &TextRun, width: f32) -> Option<Arc<TextLayout>> {
        match self.measured_run() {
            Some(measured) if measured == run => self.laid_within(width),
            _ => None,
        }
    }

    pub(crate) fn measured_run(&self) -> Option<&TextRun> {
        match &self.spec {
            MeasureSpec::Text { run, inset, .. } if *inset == Size::ZERO => Some(run),
            _ => None,
        }
    }

    pub(crate) fn laid_within(&self, width: f32) -> Option<Arc<TextLayout>> {
        self.sizes
            .iter()
            .filter_map(|measured| measured.laid.as_ref())
            .find(|laid| laid.serves_width(width))
            .cloned()
    }
}

#[derive(Debug)]
struct Measured {
    key: Option<u32>,
    size: Size,
    laid: Option<Arc<TextLayout>>,
}

fn note(requested_widths: &mut Widened<Option<u32>>, key: Option<u32>) {
    if requested_widths.contains(&key) {
        return;
    }
    if requested_widths.len() >= KEPT_WIDTHS {
        requested_widths.remove(0);
    }
    requested_widths.push(key);
}

fn measured_size(run: &TextRun, inset: Size, laid: Option<&TextLayout>) -> Size {
    let text = laid.map_or(Size::new(0.0, run.line_height), TextLayout::size);
    Size::new(text.width + inset.width, text.height + inset.height)
}

fn remember(sizes: &mut Widened<Measured>, measured: Measured) {
    if sizes.len() >= KEPT_WIDTHS {
        sizes.remove(0);
    }
    sizes.push(measured);
}
