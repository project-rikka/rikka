use crate::error::{Error, Result};
use crate::ops::Resampling;
use crate::ops::resizing::{OutputFormat, Resizing};
use crate::picture::Picture;
use crate::rewrite::rewritable;
use crate::work::Work;

impl Picture {
    pub fn resized_since(
        &self,
        resized: &Picture,
        changed: [u32; 4],
        resampling: Resampling,
    ) -> Result<Picture> {
        let (frame, earlier) = (self.frame(), resized.frame());
        let stored = frame.depth.storage(self.imaging().formats());
        if earlier.plane.format() != stored || earlier.mode != frame.mode {
            return Err(Error::Invalid(
                "earlier resize has a different source shape".into(),
            ));
        }
        let [left, top, across, down] = changed;
        let inside = left
            .checked_add(across)
            .is_some_and(|right| right <= self.width())
            && top
                .checked_add(down)
                .is_some_and(|bottom| bottom <= self.height());
        if !inside {
            return Err(Error::Invalid(format!(
                "{across}x{down} at {left},{top} is outside the {}x{} picture",
                self.width(),
                self.height()
            )));
        }
        let extent = earlier.extent();
        let resizing = Resizing::new(frame, extent, resampling.kernel());
        let Some(area) = resizing.affected_range(changed) else {
            return Ok(Picture::from_frame(
                self.imaging(),
                frame.with_plane(earlier.plane.clone()),
            ));
        };
        let mut work = Work::new(self.imaging());
        let [columns, rows] = &area;
        let rewritten = [
            columns.start,
            rows.start,
            columns.end - columns.start,
            rows.end - rows.start,
        ];
        let out = rewritable(&mut work, &earlier.plane, stored, rewritten)?;
        resizing.render(&mut work, area, &out, OutputFormat::Storage)?;
        work.finish()?;
        Ok(Picture::from_frame(self.imaging(), frame.with_plane(out)))
    }
}
