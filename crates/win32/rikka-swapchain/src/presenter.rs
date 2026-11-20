use std::time::Duration;

use ash::vk;
use rikka_renderer::{
    Acquire, Acquired, Buffering, FrameStart, Present, Recorded, RenderOutput, Renderer, Result,
    Scene, Shown, Surface, SurfaceConfig, SurfaceFit, Swapchain, SwapchainConfig, SwapchainFrame,
    SwapchainOutput,
};
use rikka_types::Rect;

use crate::timing::{Presented, Timing};

pub struct Presenter {
    surface: Surface,
    swapchain: Option<Swapchain>,
    output: SwapchainOutput,
    images: u32,
    size: (u32, u32),
    planned: Option<SurfaceFit>,
    held: Option<u32>,
    outdated: bool,
    timing: Timing,
}

#[derive(Debug)]
pub struct Image(u32);

impl Presenter {
    pub fn new(window: isize, renderer: &mut Renderer, config: SurfaceConfig) -> Result<Presenter> {
        let surface = Surface::new(renderer.gpu().device().clone(), window)?;
        tracing::debug!("presenting through a Vulkan swapchain");
        Ok(Presenter {
            surface,
            swapchain: None,
            output: renderer.create_swapchain_output(),
            images: match config.buffering {
                Buffering::Triple => 3,
                Buffering::Double => 2,
            },
            size: (1, 1),
            planned: None,
            held: None,
            outdated: false,
            timing: Timing::new(),
        })
    }

    pub fn resize(&mut self, size: (u32, u32)) {
        self.size = size;
    }

    pub fn take_presented(&mut self) -> Option<Presented> {
        self.timing.take()
    }

    fn fit(&self) -> Result<Option<SurfaceFit>> {
        self.surface.fit(vk::Extent2D {
            width: self.size.0,
            height: self.size.1,
        })
    }

    fn wanted(&self, fit: SurfaceFit) -> SwapchainConfig {
        SwapchainConfig {
            extent: fit.extent,
            images: self.images,
        }
    }

    fn holds(&self, fit: SurfaceFit) -> bool {
        !self.outdated
            && self.swapchain.as_ref().is_some_and(|swapchain| {
                swapchain.config() == self.wanted(fit) && swapchain.is_readable() == fit.readable
            })
    }

    fn recreate(&mut self, fit: SurfaceFit) -> Result<()> {
        self.held = None;
        self.outdated = false;
        let old = self.swapchain.take();
        self.swapchain = Some(Swapchain::new(&self.surface, self.wanted(fit), old)?);
        Ok(())
    }
}

impl Present for Presenter {
    type Image = Image;

    fn start(&mut self, renderer: &mut Renderer, scene: &mut Scene) -> Result<FrameStart> {
        let Some(fit) = self.fit()? else {
            self.planned = None;
            return Ok(FrameStart::Idle);
        };
        self.planned = Some(fit);
        renderer.start_swapchain(scene, &self.output, fit)
    }

    fn acquire_image(&mut self, timeout: Duration) -> Result<Acquired<Image>> {
        let Some(planned) = self.planned else {
            return Ok(Acquired::Stale);
        };
        if !self.holds(planned) {
            if self.fit()? != Some(planned) {
                return Ok(Acquired::Stale);
            }
            self.recreate(planned)?;
        }
        if let Some(index) = self.held.take() {
            return Ok(Acquired::Image(Image(index)));
        }
        let Some(swapchain) = self.swapchain.as_mut() else {
            return Ok(Acquired::Stale);
        };
        match swapchain.acquire(timeout)? {
            Acquire::Image { index, suboptimal } => {
                self.outdated |= suboptimal;
                Ok(Acquired::Image(Image(index)))
            }
            Acquire::Timeout => Ok(Acquired::NotReady),
            Acquire::Outdated => {
                self.outdated = true;
                Ok(Acquired::Stale)
            }
        }
    }

    fn record_image(
        &mut self,
        renderer: &mut Renderer,
        scene: &mut Scene,
        image: Image,
    ) -> Result<Recorded> {
        let Image(index) = image;
        let Some(swapchain) = self.swapchain.as_mut() else {
            return Ok(Recorded::deferred());
        };
        let draw = renderer.swapchain_draw(&mut self.output, swapchain, index);
        let frame = match renderer.draw(scene, draw) {
            Ok(frame) => frame,
            Err(error) => {
                self.outdated = true;
                return Err(error);
            }
        };
        match frame {
            SwapchainFrame::Submitted(output) => Ok(Recorded::new(output, index)),
            SwapchainFrame::Untouched(output) => {
                self.held = Some(index);
                Ok(match output.needs_full_redraw {
                    true => Recorded::deferred(),
                    false => Recorded::default(),
                })
            }
        }
    }

    fn present(&mut self, recorded: Recorded) -> Result<RenderOutput> {
        let Some(index) = recorded.image() else {
            return Ok(recorded.into_output());
        };
        let output = recorded.into_output();
        let Some(swapchain) = self.swapchain.as_mut() else {
            return Ok(output);
        };
        let extent = swapchain.extent();
        let damage: Vec<vk::RectLayerKHR> = output
            .damage
            .iter()
            .filter_map(|area| rect_layer(*area, extent))
            .collect();
        match swapchain.present(index, &damage) {
            Ok(Shown::Current) => {}
            Ok(Shown::Suboptimal | Shown::Outdated) => self.outdated = true,
            Err(error) => {
                self.outdated = true;
                return Err(error);
            }
        }
        self.timing.submitted();
        Ok(output)
    }
}

fn rect_layer(area: Rect, extent: vk::Extent2D) -> Option<vk::RectLayerKHR> {
    let left = area.min_x().floor().max(0.0) as u32;
    let top = area.min_y().floor().max(0.0) as u32;
    let right = (area.max_x().ceil().max(0.0) as u32).min(extent.width);
    let bottom = (area.max_y().ceil().max(0.0) as u32).min(extent.height);
    (right > left && bottom > top).then(|| vk::RectLayerKHR {
        offset: vk::Offset2D {
            x: left as i32,
            y: top as i32,
        },
        extent: vk::Extent2D {
            width: right - left,
            height: bottom - top,
        },
        layer: 0,
    })
}

impl std::fmt::Debug for Presenter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Presenter")
            .field("size", &self.size)
            .field("swapchain", &self.swapchain)
            .finish_non_exhaustive()
    }
}
