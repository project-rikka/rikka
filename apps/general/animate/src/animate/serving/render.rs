use std::time::Duration;

use animate_assistant::{RenderRequest, RenderView};
use animate_cast::Resolution;
use animate_stage::{Capture, CaptureResult, CaptureView, Picture};
use rikka_app::prelude::*;
use rikka_image::{Image as Raster, Png};
use rikka_tool::{Image, Media, Reply};
use rikka_types::PixelSize;
use tokio::sync::oneshot;

use super::camera::shot_of;
use super::{ReplySender, ServeError, ToolResult};
use crate::animate::Animate;

const WAIT: Duration = Duration::from_secs(60);
const WIDEST_SHEET: u32 = 1920;
const GAP: u32 = 4;

impl Animate {
    pub(super) fn render(&mut self, request: RenderRequest, reply: ReplySender) {
        let frames = match request.frames.is_empty() {
            true => vec![self.transport.frame()],
            false => request.frames,
        };
        let view = match request.view {
            RenderView::Film => CaptureView::CameraMotion,
            RenderView::View => CaptureView::Free,
            RenderView::Placed(placement) => CaptureView::Shot(shot_of(&placement)),
        };
        let columns = columns(frames.len());
        let longest = request
            .width
            .min((WIDEST_SHEET - GAP * (columns - 1)) / columns)
            .max(2);
        let size = fitted(longest, self.output.resolution());
        let (done, waiting) = oneshot::channel();
        {
            let mut stage = self.stage.lock();
            stage.capture(Capture {
                frames,
                view,
                size,
                done,
            });
            stage.watchers().repaint();
        }
        spawn(deliver(waiting, reply, size), |_: &mut Animate, (): ()| {
            skip_view()
        });
    }
}

async fn deliver(
    waiting: oneshot::Receiver<CaptureResult>,
    mut reply: ReplySender,
    size: PixelSize,
) {
    let captured = tokio::select! {
        captured = tokio::time::timeout(WAIT, waiting) => captured,
        () = reply.closed() => return,
    };
    let result = match captured {
        Ok(Ok(Ok(pictures))) => tokio::task::spawn_blocking(move || sheet(&pictures, size))
            .await
            .unwrap_or_else(|error| Err(ServeError::ContactSheetTask(error))),
        Ok(Ok(Err(error))) => Err(ServeError::Capture(error)),
        Ok(Err(_)) => Err(ServeError::StageClosed),
        Err(_) => Err(ServeError::RenderTimeout),
    };
    let _ = reply.send(result);
}

fn fitted(longest: u32, output: Resolution) -> PixelSize {
    let (across, down) = (output.width(), output.height());
    match output.is_portrait() {
        true => PixelSize::new((longest * across / down).max(1), longest),
        false => PixelSize::new(longest, (longest * down / across).max(1)),
    }
}

fn columns(count: usize) -> u32 {
    let mut columns = 1u32;
    while (columns * columns) < count as u32 {
        columns += 1;
    }
    columns
}

fn sheet(pictures: &[Picture], size: PixelSize) -> ToolResult {
    let PixelSize { width, height } = size;
    let columns = columns(pictures.len()).min(pictures.len().max(1) as u32);
    let rows = (pictures.len() as u32).div_ceil(columns);
    let across = columns * width + (columns - 1) * GAP;
    let down = rows * height + rows.saturating_sub(1) * GAP;
    let mut canvas = vec![0u8; (across * down * 4) as usize];
    for pixel in canvas.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    let row_bytes = (width * 4) as usize;
    for (at, picture) in pictures.iter().enumerate() {
        let at = at as u32;
        let left = (at % columns) * (width + GAP);
        let top = (at / columns) * (height + GAP);
        for line in 0..height {
            let from = (line * width * 4) as usize;
            let into = (((top + line) * across + left) * 4) as usize;
            canvas[into..into + row_bytes].copy_from_slice(&picture.rgba[from..from + row_bytes]);
        }
    }
    let png = Raster::from_rgba8(across, down, canvas)
        .and_then(|raster| raster.encode(Png))
        .map_err(ServeError::ContactSheet)?;
    let frames: Vec<String> = pictures
        .iter()
        .map(|picture| picture.frame.to_string())
        .collect();
    let report = match pictures.len() {
        1 => format!("Frame {}, {width}×{height}.", frames[0]),
        _ => format!(
            "Frames {} in {columns} columns, left to right and top to bottom, each {width}×{height}.",
            frames.join(", ")
        ),
    };
    Ok(Reply::new(report).display(Image::inline(Media::Png, png)))
}
