use paint_assistant::{Area, MeasureOrder, Rect, Statistics, sample_line};
use paint_document::{LayerId, Region};
use rikka_image_edit::Resampling;
use rikka_tool::Reply;

use super::pixels::{clipped, contains, rgba, source, whole};
use super::{Answer, Paint, ServeResult, layers, reply};
use crate::tab::WorkerView;

const MAX_MEASURED: u64 = 4_000_000;

enum MeasureArea {
    Whole,
    Selection,
    Region(Rect),
}

struct MeasureRequest {
    name: String,
    over: Option<MeasureArea>,
    layer: Option<LayerId>,
    points: Vec<(u32, u32)>,
    radius: u32,
}

impl Paint {
    pub(super) fn measure(&mut self, index: usize, order: MeasureOrder, answer: Answer) {
        let request = match self.measure_request(index, order) {
            Ok(request) => request,
            Err(refusal) => return reply(answer, Err(refusal)),
        };
        let read = self.tabs[index].read(move |view| reply(answer, measured(view, &request)));
        if !read {
            tracing::warn!("measure request for document without worker");
        }
    }

    fn measure_request(&self, index: usize, order: MeasureOrder) -> Result<MeasureRequest, String> {
        let tab = self.loaded(index)?;
        let facts = tab.facts().ok_or("the document is not open")?;
        let selected = !tab.selection().region().is_empty();
        let over = match order.area {
            None => None,
            Some(Area::Chosen) if selected => Some(MeasureArea::Selection),
            Some(Area::Chosen | Area::Whole) => Some(MeasureArea::Whole),
            Some(Area::Selection) if selected => Some(MeasureArea::Selection),
            Some(Area::Selection) => {
                return Err(format!(
                    "nothing is selected in \"{}\"; measure \"whole\" or a region",
                    self.handle(index)
                ));
            }
            Some(Area::Region(region)) => Some(MeasureArea::Region(clipped(
                region,
                facts.width,
                facts.height,
            )?)),
        };
        if let Some((x, y)) = order
            .points
            .iter()
            .find(|(x, y)| *x >= facts.width || *y >= facts.height)
        {
            return Err(format!(
                "({x}, {y}) lies outside the {}×{} picture",
                facts.width, facts.height
            ));
        }
        Ok(MeasureRequest {
            name: self.handle(index),
            over,
            layer: order
                .layer
                .as_ref()
                .map(|request| layers::find(tab.layers(), request).map(|layer| layer.id))
                .transpose()?,
            points: order.points,
            radius: order.radius,
        })
    }
}

fn measured(view: WorkerView<'_>, request: &MeasureRequest) -> ServeResult {
    let document = view.document;
    let mut segments = vec![format!("\"{}\"", request.name)];
    if let Some(layer) = request.layer {
        segments.push(format!(" layer {} alone", layer.index()));
    }
    segments.push(", sRGB 0–255.".to_owned());
    if let Some(over) = &request.over {
        let (width, height) = (document.width(), document.height());
        let selection: Option<Region> = match over {
            MeasureArea::Selection => Some(document.selection().clone()),
            MeasureArea::Whole | MeasureArea::Region(_) => None,
        };
        let bounds = match over {
            MeasureArea::Whole => whole(width, height),
            MeasureArea::Region(region) => *region,
            MeasureArea::Selection => {
                let area = selection
                    .as_ref()
                    .and_then(Region::bounds)
                    .ok_or("nothing is selected")?;
                Rect {
                    left: area.left,
                    top: area.top,
                    width: area.width,
                    height: area.height,
                }
            }
        };
        let total = u64::from(bounds.width) * u64::from(bounds.height);
        let factor = match total > MAX_MEASURED {
            true => (total as f64 / MAX_MEASURED as f64).sqrt(),
            false => 1.0,
        };
        let size = (
            ((f64::from(bounds.width) / factor).floor() as u32).max(1),
            ((f64::from(bounds.height) / factor).floor() as u32).max(1),
        );
        let picture = source(document, request.layer)?;
        let pixels = rgba(&picture, bounds, size, Resampling::Nearest)?;
        let inside = |x: u32, y: u32| {
            selection.as_ref().is_none_or(|region| {
                let at = |along: u32, extent: u32, start: u32, sampled: u32| {
                    start + (u64::from(along) * u64::from(extent) / u64::from(sampled)) as u32
                };
                contains(
                    region,
                    at(x, bounds.width, bounds.left, size.0),
                    at(y, bounds.height, bounds.top, size.1),
                )
            })
        };
        segments.push(format!(
            "\nArea x {}–{}, y {}–{}{}",
            bounds.left,
            bounds.left + bounds.width,
            bounds.top,
            bounds.top + bounds.height,
            match over {
                MeasureArea::Selection => ", selected pixels only",
                MeasureArea::Whole => ", the whole picture",
                MeasureArea::Region(_) => "",
            }
        ));
        if factor > 1.0 {
            segments.push(format!(", sampled every {factor:.1} px"));
        }
        segments.push(":\n".to_owned());
        segments.push(match Statistics::measure(&pixels, size.0, inside) {
            Some(statistics) => statistics.to_string(),
            None => "every pixel there is transparent".to_owned(),
        });
    }
    if !request.points.is_empty() {
        segments.push(format!(
            "\nColours{}:",
            match request.radius {
                0 => String::new(),
                radius => format!(" averaged over {0}×{0} px", radius * 2 + 1),
            }
        ));
        for (x, y) in &request.points {
            let color = document
                .sample(*x, *y, request.radius * 2 + 1, request.layer)
                .map_err(|error| error.to_string())?;
            segments.push(format!("\n{}", sample_line(*x, *y, color)));
        }
    }
    Ok(Reply::new(segments.concat()))
}
