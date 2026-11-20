use std::fmt;
use std::time::{Duration, Instant};

use rikka_compositor_input::DeviceSettings;
use rikka_compositor_reactor::{Reactor, Token};
use rikka_compositor_surface::{Presentation, PresentationKind, Refresh};
use rikka_compositor_types::OutputId;
use rikka_renderer::{OffscreenTarget, Renderer};
use tracing::Level;

use super::{Backend, Extent};
use crate::error::{Error, Result};
use crate::outputs::{Head, Mode, OutputConfig, Outputs};
use crate::programs::COMPOSITOR;
use crate::render::{Snapshot, monotonic};
use crate::state::{Compositor, Parts};

pub(crate) struct Headless {
    reactor: Reactor<Compositor>,
    renderer: Renderer,
    output: OutputId,
    mode: Mode,
    target: Option<OffscreenTarget>,
    drawn: Option<Instant>,
    due: Option<Token>,
    sequence: u64,
}

impl Headless {
    pub(crate) fn start(
        reactor: &Reactor<Compositor>,
        outputs: &mut Outputs,
        extent: Extent,
        millihertz: u32,
    ) -> Result<Self> {
        let extent = extent.clamped();
        let renderer = Renderer::new(
            &rikka_gpu::Gpu::builder()
                .app_name(COMPOSITOR)
                .upload_queue()
                .build()
                .map_err(Error::Renderer)?,
        )
        .map_err(Error::Renderer)?;
        let mode = Mode {
            width: extent.width,
            height: extent.height,
            refresh: millihertz,
        };
        let output = outputs.connect(Head {
            name: "headless-0".to_owned(),
            make: "Rikka".to_owned(),
            model: "Headless".to_owned(),
            serial: None,
            size_mm: (0, 0),
            modes: vec![mode],
            scale: 1.0,
            hdr: None,
        });
        Ok(Self {
            reactor: reactor.clone(),
            renderer,
            output,
            mode,
            target: None,
            drawn: None,
            due: None,
            sequence: 0,
        })
    }

    fn frame(compositor: &mut Compositor) {
        if let Some((headless, mut parts)) = compositor.backend::<Headless>() {
            headless.due = None;
            headless.draw(&mut parts);
        }
    }

    fn draw(&mut self, parts: &mut Parts<'_>) {
        let Some(target) = &mut self.target else {
            return;
        };
        let started = Instant::now();
        self.drawn = Some(started);
        let Some(scene) = parts.compose(self.output, &mut self.renderer) else {
            return;
        };
        let composed = started.elapsed();
        let rendered = match self.renderer.draw(scene, &mut *target) {
            Ok(drawn) => {
                if drawn.needs_full_redraw {
                    parts.scenes.damage_whole(self.output);
                    parts.frames.request(self.output);
                }
                drawn.rendered()
            }
            Err(error) => {
                tracing::warn!("cannot render headless output: {error}");
                false
            }
        };
        let frame = self.renderer.submitted_frame();
        if rendered && tracing::enabled!(Level::TRACE) {
            parts.scenes.watch_frame(frame);
        }
        tracing::trace!(
            work = ?started.elapsed(),
            composed = ?composed,
            rendered,
            frame,
            "headless frame"
        );
        let presentation = Presentation {
            time: monotonic(),
            refresh: Refresh::Fixed(self.interval()),
            sequence: self.sequence + 1,
            flags: PresentationKind::empty(),
        };
        if rendered {
            self.sequence += 1;
        }
        parts.composited(self.output, frame, rendered, &presentation);
    }

    fn interval(&self) -> Duration {
        self.mode.interval()
    }
}

impl Backend for Headless {
    fn configure(&mut self, parts: &mut Parts<'_>, configs: &[(OutputId, OutputConfig)]) -> bool {
        if configs
            .iter()
            .any(|(output, config)| *output != self.output || config.mode != self.mode)
        {
            return false;
        }
        for (output, config) in configs {
            if !config.enabled {
                self.target = None;
                parts.scenes.forget(*output, &mut self.renderer);
                parts.outputs.apply(*output, *config);
                continue;
            }
            if self.target.is_none() {
                match self
                    .renderer
                    .create_target(self.mode.width, self.mode.height)
                {
                    Ok(target) => self.target = Some(target),
                    Err(error) => {
                        tracing::warn!("cannot create headless output: {error}");
                        return false;
                    }
                }
            }
            if let Some(target) = &mut self.target {
                target.set_transform(config.transform);
            }
            parts.outputs.apply(
                *output,
                OutputConfig {
                    hdr: false,
                    ..*config
                },
            );
            parts.frames.request(*output);
        }
        true
    }

    fn schedule(&mut self, output: OutputId) {
        if output != self.output || self.due.is_some() || self.target.is_none() {
            return;
        }
        let now = Instant::now();
        let at = self
            .drawn
            .map_or(now, |drawn| (drawn + self.interval()).max(now));
        self.due = Some(self.reactor.timer(at, Headless::frame));
    }

    fn configure_devices(&mut self, _: DeviceSettings) {}

    fn gpu_reached(&mut self, finished: u64, at: Instant) {
        tracing::trace!(frame = finished, latency = ?at.elapsed(), "headless frame reached");
    }

    fn cursor_changed(&mut self, parts: &mut Parts<'_>) {
        parts.frames.request(self.output);
    }

    fn switches_vt(&self) -> bool {
        false
    }

    fn renderer(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    fn snapshot(&mut self, output: OutputId) -> Option<Snapshot> {
        let target = self.target.as_mut().filter(|_| output == self.output)?;
        let (width, height) = (target.width(), target.height());
        match self.renderer.read_pixels(target) {
            Ok(pixels) => Some(Snapshot::new(width, height, pixels)),
            Err(error) => {
                tracing::warn!("cannot read back headless output: {error}");
                None
            }
        }
    }
}

impl fmt::Debug for Headless {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Headless")
            .field("output", &self.output)
            .field("mode", &self.mode)
            .field("due", &self.due)
            .finish_non_exhaustive()
    }
}
