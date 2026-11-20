use std::sync::{Arc, Mutex};

use code_meaning::meaning;
use rikka_barcode::{Barcode, Picture};
use rikka_seam::{Request, SeamClient};
use rikka_types::{Point, Rect};
use shell_kit::detached;

use crate::error::Result;
use crate::frozen::{Frozen, Panel};
use crate::scan::codes::{Code, Scope};
use crate::service::{CAPABILITY, SCANNED};

const MARGIN: f32 = 6.0;
const THINNEST: f32 = 24.0;

pub(crate) struct ScanResult {
    pub(crate) frozen: Arc<Frozen>,
    pub(crate) scope: Scope,
    pub(crate) read: Result<Vec<Code>>,
}

#[derive(Clone, Default)]
pub(crate) struct ScanResults(Arc<Mutex<Vec<ScanResult>>>);

impl ScanResults {
    #[must_use]
    pub(crate) fn take(&self) -> Vec<ScanResult> {
        self.0
            .lock()
            .map(|mut results| std::mem::take(&mut *results))
            .unwrap_or_default()
    }

    fn deliver(&self, scan_result: ScanResult, seam: &SeamClient) {
        if let Ok(mut results) = self.0.lock() {
            results.push(scan_result);
        }
        seam.notify(Request::new(CAPABILITY, SCANNED));
    }
}

impl std::fmt::Debug for ScanResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanResults").finish_non_exhaustive()
    }
}

pub(crate) fn look(frozen: Arc<Frozen>, scope: Scope, scan_results: ScanResults, seam: SeamClient) {
    detached("rikka-capture-scan", move || {
        let read = match scope {
            Scope::Screen => Ok(screen(&frozen)),
            Scope::Area(framing) => area_of(&frozen, framing.area),
        };
        if let Err(error) = &read {
            tracing::warn!("failed to read codes: {error}");
        }
        scan_results.deliver(
            ScanResult {
                frozen,
                scope,
                read,
            },
            &seam,
        );
    });
}

fn screen(frozen: &Frozen) -> Vec<Code> {
    std::thread::scope(|scope| {
        let reading: Vec<_> = frozen
            .panels()
            .iter()
            .map(|panel| (panel, scope.spawn(move || panel_codes(panel))))
            .collect();
        let mut codes = Vec::new();
        for (panel, read) in reading {
            match read.join() {
                Ok(Ok(read)) => codes.extend(read),
                Ok(Err(error)) => {
                    tracing::warn!("failed to scan `{}` for codes: {error}", panel.name);
                }
                Err(_) => tracing::warn!("failed to scan `{}` for codes", panel.name),
            }
        }
        codes
    })
}

fn panel_codes(panel: &Panel) -> Result<Vec<Code>> {
    let picture = Picture::rgba(panel.width, panel.height, &panel.pixels)?;
    let scale = panel.scale();
    let origin = Point::new(panel.bounds.min_x(), panel.bounds.min_y());
    Ok(rikka_barcode::read(&picture)?
        .into_iter()
        .map(|barcode| placed(barcode, origin, scale))
        .collect())
}

fn area_of(frozen: &Frozen, area: Rect) -> Result<Vec<Code>> {
    let area = area.intersection(&frozen.bounds()).unwrap_or(area);
    let image = frozen.crop(area)?;
    let pixels = image.rgba8();
    let picture = Picture::rgba(image.width(), image.height(), &pixels)?;
    let scale = image.width() as f32 / area.width().max(1.0);
    let origin = Point::new(area.min_x(), area.min_y());
    Ok(rikka_barcode::read_framed(&picture)?
        .into_iter()
        .map(|barcode| placed(barcode, origin, scale))
        .collect())
}

fn placed(barcode: Barcode, origin: Point, scale: f32) -> Code {
    let code_bounds = barcode.bounds();
    let area = Rect::new(
        origin.x + code_bounds.min_x() / scale,
        origin.y + code_bounds.min_y() / scale,
        code_bounds.width() / scale,
        code_bounds.height() / scale,
    );
    let thicken = ((THINNEST - area.height()) / 2.0).max(0.0);
    let area = Rect::new(
        area.min_x() - MARGIN,
        area.min_y() - MARGIN - thicken,
        area.width() + MARGIN * 2.0,
        area.height() + (MARGIN + thicken) * 2.0,
    );
    Code {
        format: barcode.format(),
        meaning: meaning(barcode.text()),
        text: barcode.text().to_owned(),
        area,
    }
}
