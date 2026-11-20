use crate::error::Result;
use crate::ops::{Resampling, Resize};
use crate::picture::{Frame, Picture};
use crate::recipe::develop::{apply, store};
use crate::recipe::{Recipe, Step};
use crate::work::Work;

pub struct Preview {
    source: Picture,
    proxies: Vec<(u32, Picture)>,
    level: u32,
    cached_steps: Vec<(Step, Frame)>,
}

impl Preview {
    #[must_use]
    pub fn new(source: Picture) -> Preview {
        Preview {
            source,
            proxies: Vec::new(),
            level: 0,
            cached_steps: Vec::new(),
        }
    }

    #[must_use]
    pub fn source(&self) -> &Picture {
        &self.source
    }

    pub fn render(&mut self, recipe: &Recipe, scale: f32) -> Result<Picture> {
        let level = level_for(scale);
        let proxy = self.proxy(level)?;
        if level != self.level {
            self.cached_steps.clear();
            self.level = level;
        }
        let reused = self
            .cached_steps
            .iter()
            .zip(recipe.steps())
            .take_while(|((cached, _), step)| cached == *step)
            .count();
        self.cached_steps.truncate(reused);
        let factor = proxy.width() as f32 / self.source.width() as f32;
        let mut work = Work::new(proxy.imaging());
        let mut frame = self
            .cached_steps
            .last()
            .map_or_else(|| proxy.frame().clone(), |(_, frame)| frame.clone());
        for (index, step) in recipe.steps().iter().enumerate().skip(reused) {
            frame = apply(&mut work, &frame, step, factor, index as u32)?;
            self.cached_steps.push((step.clone(), frame.clone()));
        }
        let stored = store(&mut work, &frame)?;
        work.finish()?;
        Ok(Picture::from_frame(proxy.imaging(), stored))
    }

    fn proxy(&mut self, level: u32) -> Result<Picture> {
        if level == 0 {
            return Ok(self.source.clone());
        }
        if let Some((_, proxy)) = self.proxies.iter().find(|(cached, _)| *cached == level) {
            return Ok(proxy.clone());
        }
        let divisor = 1u32 << level;
        let width = self.source.width().div_ceil(divisor).max(1);
        let height = self.source.height().div_ceil(divisor).max(1);
        let reduced = self.source.develop(
            &Recipe::new().then(Resize::new(width, height).resampling(Resampling::BicubicSmoother)),
        )?;
        self.proxies.push((level, reduced.clone()));
        Ok(reduced)
    }
}

fn level_for(scale: f32) -> u32 {
    if scale.is_nan() || scale <= 0.0 || scale >= 1.0 {
        return 0;
    }
    (1.0 / scale).log2().floor().clamp(0.0, 12.0) as u32
}

impl std::fmt::Debug for Preview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preview")
            .field("source", &self.source)
            .field("level", &self.level)
            .field("cached", &self.cached_steps.len())
            .finish()
    }
}
