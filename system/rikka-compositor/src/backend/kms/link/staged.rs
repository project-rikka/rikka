use std::collections::{HashMap, VecDeque};

use rikka_compositor_geometry::{Point, Rect as Area, Size};
use rikka_compositor_kms::{Framebuffer, FramebufferId, Output, Rect};
use rikka_compositor_pixels::{write_deep_rows, write_rows};
use rikka_renderer::{
    CopySink, OffscreenTarget, Readback, RenderOutput, Renderer, Scene, SharedRows, TargetFormat,
    TexelRect,
};

use super::LinkError;

const REMEMBERED: usize = 4;

pub(in crate::backend::kms) struct Staged {
    target: OffscreenTarget,
    format: TargetFormat,
    sink: Sink,
    history: VecDeque<Vec<Rect>>,
    copying: Option<Copying>,
}

enum Sink {
    Mapped(Readback),
    Rows(HashMap<FramebufferId, SharedRows>),
}

pub(in crate::backend::kms) struct Copying {
    pub(in crate::backend::kms) framebuffer: Framebuffer,
    pub(in crate::backend::kms) frame: u64,
    pub(in crate::backend::kms) areas: Vec<Rect>,
    ready_at: u64,
}

impl Staged {
    pub(super) fn mapped(
        output: &Output,
        renderer: &mut Renderer,
        format: TargetFormat,
    ) -> Result<Self, LinkError> {
        let readback = renderer.create_readback();
        Self::new(output, renderer, Sink::Mapped(readback), format)
    }

    pub(super) fn rows(
        output: &Output,
        renderer: &mut Renderer,
        rows: Vec<SharedRows>,
        format: TargetFormat,
    ) -> Result<Self, LinkError> {
        let rows = output
            .swapchain()
            .framebuffers()
            .map(Framebuffer::id)
            .zip(rows)
            .collect();
        Self::new(output, renderer, Sink::Rows(rows), format)
    }

    fn new(
        output: &Output,
        renderer: &mut Renderer,
        sink: Sink,
        format: TargetFormat,
    ) -> Result<Self, LinkError> {
        let mode = output.mode();
        Ok(Self {
            target: renderer.create_target_as(mode.width(), mode.height(), format)?,
            format,
            sink,
            history: VecDeque::new(),
            copying: None,
        })
    }

    pub(in crate::backend::kms) fn target(&mut self) -> &mut OffscreenTarget {
        &mut self.target
    }

    pub(in crate::backend::kms) fn busy(&self) -> bool {
        self.copying.is_some()
    }

    pub(in crate::backend::kms) fn draw(
        &mut self,
        renderer: &mut Renderer,
        scene: &mut Scene,
        framebuffer: &Framebuffer,
        age: u32,
    ) -> rikka_renderer::Result<RenderOutput> {
        let missed = self.missed(age);
        let sink = match &mut self.sink {
            Sink::Mapped(readback) => CopySink::Readback(readback),
            Sink::Rows(rows) => CopySink::Rows(rows.get(&framebuffer.id()).ok_or_else(|| {
                rikka_renderer::Error::Invalid("scanout buffer has no CPU mapping".into())
            })?),
        };
        renderer.draw(scene, self.target.copied_into(sink, &missed))
    }

    fn missed(&self, age: u32) -> Vec<TexelRect> {
        let age = age as usize;
        if age == 0 || age > (self.history.len() + 1).min(REMEMBERED) {
            return vec![TexelRect::new(
                [0, 0],
                [self.target.width(), self.target.height()],
            )];
        }
        let mut areas: Vec<TexelRect> = Vec::new();
        for area in self.history.iter().rev().take(age - 1).flatten() {
            let texels = TexelRect::new(
                [area.x.max(0) as u32, area.y.max(0) as u32],
                [area.width, area.height],
            );
            if !areas.contains(&texels) {
                areas.push(texels);
            }
        }
        areas
    }

    pub(in crate::backend::kms) fn buffer_damage(
        &mut self,
        damage: Vec<Rect>,
        age: u32,
    ) -> Vec<Rect> {
        self.history.push_back(damage);
        while self.history.len() > REMEMBERED {
            self.history.pop_front();
        }
        let age = age as usize;
        if age == 0 || age > self.history.len() {
            return vec![Rect::sized(self.target.width(), self.target.height())];
        }
        let mut areas: Vec<Rect> = Vec::new();
        for area in self.history.iter().rev().take(age).flatten() {
            if !areas.contains(area) {
                areas.push(*area);
            }
        }
        areas
    }

    pub(in crate::backend::kms) fn copying(
        &mut self,
        framebuffer: Framebuffer,
        frame: u64,
        areas: Vec<Rect>,
        ready_at: u64,
    ) {
        self.copying = Some(Copying {
            framebuffer,
            frame,
            areas,
            ready_at,
        });
    }

    pub(in crate::backend::kms) fn copied(
        &mut self,
        renderer: &mut Renderer,
        finished: u64,
        output: &mut Output,
    ) -> Option<Result<Copying, Copying>> {
        if self
            .copying
            .as_ref()
            .is_none_or(|copying| copying.ready_at > finished)
        {
            return None;
        }
        match &mut self.sink {
            Sink::Rows(_) => self.copying.take().map(Ok),
            Sink::Mapped(readback) => {
                if !renderer.poll_readback(readback) {
                    return None;
                }
                let copying = self.copying.take()?;
                let fourcc = output.swapchain().format().fourcc;
                let write = match self.format {
                    TargetFormat::Rgba8 => write_rows,
                    TargetFormat::Rgb10a2 => write_deep_rows,
                };
                let filled = output
                    .swapchain_mut()
                    .mapped(copying.framebuffer.id())
                    .is_some_and(|mapped| {
                        let (width, height) = readback.extent();
                        let stride = mapped.stride() as usize;
                        write(
                            mapped.bytes_mut(),
                            fourcc,
                            stride,
                            (width as usize, height as usize),
                            readback.pixels(),
                            copying.areas.iter().map(|area| {
                                Area::new(
                                    Point::new(area.x, area.y),
                                    Size::new(area.width.cast_signed(), area.height.cast_signed()),
                                )
                            }),
                        )
                    });
                Some(if filled { Ok(copying) } else { Err(copying) })
            }
        }
    }

    pub(in crate::backend::kms) fn abandon(&mut self) -> Option<u64> {
        self.history.clear();
        self.copying.take().map(|copying| copying.frame)
    }
}
