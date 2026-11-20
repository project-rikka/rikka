use paint_assistant::{Framing, Grid, Panel, Showing, ViewOrder};
use paint_document::Document;
use rikka_image_edit::{Picture, Resampling};

use super::pixels::{clipped, rgba, source, whole};
use super::{Answer, Paint, ServeResult, layers, reply};
use crate::tab::WorkerView;

struct ViewRequest {
    name: String,
    framing: Framing,
    layer: Option<paint_document::LayerId>,
    grid: Option<u32>,
    outline: bool,
    compare: bool,
}

impl Paint {
    pub(super) fn view(&mut self, index: usize, order: ViewOrder, answer: Answer) {
        let request = match self.view_request(index, &order) {
            Ok(request) => request,
            Err(refusal) => return reply(answer, Err(refusal)),
        };
        let read = self.tabs[index].read(move |view| reply(answer, render_view(view, &request)));
        if !read {
            tracing::warn!("view request for document without worker");
        }
    }

    fn view_request(&self, index: usize, order: &ViewOrder) -> Result<ViewRequest, String> {
        let tab = self.loaded(index)?;
        let facts = tab.facts().ok_or("the document is not open")?;
        let region = match order.region {
            Some(region) => clipped(region, facts.width, facts.height)?,
            None => whole(facts.width, facts.height),
        };
        if order.compare && order.layer.is_some() {
            return Err("compare shows the whole visible picture; leave out `layer`".to_owned());
        }
        let layer = order
            .layer
            .as_ref()
            .map(|request| layers::find(tab.layers(), request).map(|layer| layer.id))
            .transpose()?;
        let framing = Framing::fit(region, order.longest, if order.compare { 2 } else { 1 });
        let grid = match order.grid {
            None => None,
            Some(Grid::Auto) => Some(framing.grid_step(None)?),
            Some(Grid::Every(step)) => Some(framing.grid_step(Some(step))?),
        };
        Ok(ViewRequest {
            name: self.handle(index),
            framing,
            layer,
            grid,
            outline: order.selection.unwrap_or(true),
            compare: order.compare,
        })
    }
}

fn render_view(view: WorkerView<'_>, request: &ViewRequest) -> ServeResult {
    let document: &Document = view.document;
    let now = source(document, request.layer)?;
    let framing = request.framing;
    let resampling = match framing.enlarges() {
        true => Resampling::Nearest,
        false => Resampling::BicubicSmoother,
    };
    let size = (framing.width, framing.height);
    let mut panels = Vec::new();
    if request.compare {
        let before: &Picture = view.before.ok_or(
            "nothing has been changed through these tools in this document yet, so there is no before",
        )?;
        if before.dimensions() != now.dimensions() {
            return Err(format!(
                "the last change resized the canvas from {}×{} to {}×{}; view each state instead of comparing",
                before.width(),
                before.height(),
                now.width(),
                now.height()
            ));
        }
        panels.push(Panel {
            label: Some("before".to_owned()),
            rgba: rgba(before, framing.region, size, resampling)?,
        });
    }
    panels.push(Panel {
        label: request.compare.then(|| "after".to_owned()),
        rgba: rgba(&now, framing.region, size, resampling)?,
    });
    let selection = document.selection();
    let outline = match request.outline && !selection.is_empty() {
        true => selection.outline(),
        false => Vec::new(),
    };
    let compare = match request.compare {
        true => " Left is before the last change made through these tools, right is now.",
        false => "",
    };
    let grid = request
        .grid
        .map(|step| format!(" Grid lines every {step} px, labelled in document coordinates."))
        .unwrap_or_default();
    let outlined = match outline.is_empty() {
        true => "",
        false => " The selection is outlined in black and white dashes.",
    };
    let text = format!(
        "\"{}\"{}: {}.{compare}{grid}{outlined}",
        request.name,
        match request.layer {
            Some(layer) => format!(" layer {} alone", layer.index()),
            None => String::new(),
        },
        framing.mapping()
    );
    Showing {
        framing,
        panels,
        grid: request.grid,
        outline,
        text,
    }
    .reply()
}
