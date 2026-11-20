use crate::area::Area;
use crate::document::check_memory_budget;
use crate::error::Result;
use crate::region::{Builder, Region};

#[derive(Debug)]
pub(crate) struct Raster {
    pub(crate) area: Area,
    pub(crate) values: Vec<u8>,
}

impl Raster {
    pub(crate) fn blank(area: Area) -> Result<Raster> {
        check_memory_budget(
            area.width,
            area.height,
            u64::from(area.width) * u64::from(area.height),
        )?;
        Ok(Raster {
            area,
            values: vec![0; area.width as usize * area.height as usize],
        })
    }

    pub(crate) fn rasterize(region: &Region, area: Area) -> Result<Raster> {
        let mut raster = Raster::blank(area)?;
        let (left, right) = (area.left, area.right());
        for (row, line) in raster.values.chunks_mut(area.width as usize).enumerate() {
            for span in region.row(area.top + row as u32) {
                let (start, end) = (span.start.max(left), span.end.min(right));
                if start < end {
                    line[(start - left) as usize..(end - left) as usize].fill(span.coverage);
                }
            }
        }
        Ok(raster)
    }

    pub(crate) fn row(&self, row: u32) -> &[u8] {
        let width = self.area.width as usize;
        &self.values[row as usize * width..(row as usize + 1) * width]
    }

    pub(crate) fn region(&self) -> Region {
        let mut builder = Builder::new(self.area.top);
        for row in 0..self.area.height {
            let line = self.row(row);
            let mut start = 0;
            while start < line.len() {
                let value = line[start];
                let end = line[start..]
                    .iter()
                    .position(|other| *other != value)
                    .map_or(line.len(), |length| start + length);
                builder.push(
                    self.area.left + start as u32,
                    self.area.left + end as u32,
                    value,
                );
                start = end;
            }
            builder.end_row();
        }
        builder.finish()
    }
}

pub(crate) fn widened(area: Area, by: u32, canvas: Area) -> Area {
    let left = area.left.saturating_sub(by).max(canvas.left);
    let top = area.top.saturating_sub(by).max(canvas.top);
    let right = area.right().saturating_add(by).min(canvas.right());
    let bottom = (area.top + area.height)
        .saturating_add(by)
        .min(canvas.top + canvas.height);
    Area::new(
        left,
        top,
        right.saturating_sub(left),
        bottom.saturating_sub(top),
    )
}
