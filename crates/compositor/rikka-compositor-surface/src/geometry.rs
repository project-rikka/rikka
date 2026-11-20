use rikka_compositor_geometry::{Buffer, Logical, Point, Rect, Size, Transform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Geometry {
    pub(crate) buffer: Option<Size<i32, Buffer>>,
    pub(crate) scale: i32,
    pub(crate) transform: Transform,
    pub(crate) source: Option<Rect<f64, Logical>>,
    pub(crate) destination: Option<Size<i32, Logical>>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Mapping {
    size: Size<i32, Logical>,
    buffer: Size<i32, Buffer>,
    unscaled: Size<i32, Logical>,
    scale: i32,
    transform: Transform,
    crop: Option<Crop>,
}

#[derive(Debug, Clone, Copy)]
struct Crop {
    source: Rect<f64, Logical>,
    x: f64,
    y: f64,
}

impl Geometry {
    fn unscaled(&self) -> Option<Size<i32, Logical>> {
        Some(self.buffer?.to_logical(self.scale, self.transform))
    }

    pub(crate) fn size(&self) -> Option<Size<i32, Logical>> {
        Some(self.sized(self.unscaled()?))
    }

    fn sized(&self, unscaled: Size<i32, Logical>) -> Size<i32, Logical> {
        match (self.destination, self.source) {
            (Some(destination), _) => destination,
            (None, Some(source)) => {
                Size::new((source.size.w as i32).max(1), (source.size.h as i32).max(1))
            }
            (None, None) => unscaled,
        }
    }

    pub(crate) fn source(&self) -> Option<Rect<f64, Logical>> {
        let unscaled = self.unscaled()?;
        Some(
            self.source
                .unwrap_or_else(|| Rect::from_size(unscaled.to_f64())),
        )
    }

    pub(crate) fn source_in_buffer(&self) -> Option<Rect<f64, Buffer>> {
        let area = self.unscaled()?.to_f64();
        Some(
            self.source()?
                .to_buffer(f64::from(self.scale), self.transform, &area),
        )
    }

    pub(crate) fn divisible(&self) -> bool {
        self.buffer.is_none_or(|size| {
            self.destination.is_some() || (size.w % self.scale == 0 && size.h % self.scale == 0)
        })
    }

    pub(crate) fn source_integral(&self) -> bool {
        self.destination.is_some()
            || self
                .source
                .is_none_or(|source| source.size.w.fract() == 0.0 && source.size.h.fract() == 0.0)
    }

    pub(crate) fn source_fits(&self) -> bool {
        let (Some(source), Some(unscaled)) = (self.source, self.unscaled()) else {
            return true;
        };
        let area = unscaled.to_f64();
        source.loc.x + source.size.w <= area.w && source.loc.y + source.size.h <= area.h
    }

    pub(crate) fn mapping(&self) -> Option<Mapping> {
        let buffer = self.buffer?;
        let unscaled = buffer.to_logical(self.scale, self.transform);
        let size = self.sized(unscaled);
        let crop = (self.source.is_some() || size != unscaled).then(|| {
            let source = self
                .source
                .unwrap_or_else(|| Rect::from_size(unscaled.to_f64()));
            Crop {
                source,
                x: source.size.w / f64::from(size.w),
                y: source.size.h / f64::from(size.h),
            }
        });
        Some(Mapping {
            size,
            buffer,
            unscaled,
            scale: self.scale,
            transform: self.transform,
            crop,
        })
    }
}

impl Mapping {
    pub(crate) fn size(&self) -> Size<i32, Logical> {
        self.size
    }

    pub(crate) fn buffer(&self) -> Size<i32, Buffer> {
        self.buffer
    }

    pub(crate) fn surface_to_buffer(&self, rect: Rect<i32, Logical>) -> Option<Rect<i32, Buffer>> {
        let rect = rect.intersection(Rect::from_size(self.size))?;
        let buffer = match self.crop {
            None => rect.to_buffer(self.scale, self.transform, &self.unscaled),
            Some(crop) => {
                let uncropped = Rect::new(
                    Point::new(
                        crop.source.loc.x + f64::from(rect.loc.x) * crop.x,
                        crop.source.loc.y + f64::from(rect.loc.y) * crop.y,
                    ),
                    Size::new(
                        f64::from(rect.size.w) * crop.x,
                        f64::from(rect.size.h) * crop.y,
                    ),
                );
                outward(uncropped.to_buffer(
                    f64::from(self.scale),
                    self.transform,
                    &self.unscaled.to_f64(),
                ))
            }
        };
        buffer.intersection(Rect::from_size(self.buffer))
    }

    pub(crate) fn buffer_to_surface(&self, rect: Rect<i32, Buffer>) -> Option<Rect<i32, Logical>> {
        let rect = rect.intersection(Rect::from_size(self.buffer))?;
        let local = match self.crop {
            None => {
                let turned = rect.transformed_in(self.transform.inverse(), &self.buffer);
                let scale = self.scale;
                Rect::from_extremities(
                    (
                        turned.loc.x.div_euclid(scale),
                        turned.loc.y.div_euclid(scale),
                    ),
                    (
                        ceil_div(turned.loc.x + turned.size.w, scale),
                        ceil_div(turned.loc.y + turned.size.h, scale),
                    ),
                )
            }
            Some(crop) => {
                let uncropped = rect.to_f64().to_logical(
                    f64::from(self.scale),
                    self.transform,
                    &self.buffer.to_f64(),
                );
                outward(Rect::new(
                    Point::new(
                        (uncropped.loc.x - crop.source.loc.x) / crop.x,
                        (uncropped.loc.y - crop.source.loc.y) / crop.y,
                    ),
                    Size::new(uncropped.size.w / crop.x, uncropped.size.h / crop.y),
                ))
            }
        };
        local.intersection(Rect::from_size(self.size))
    }
}

pub(crate) fn floor(value: f64) -> i32 {
    let truncated = value as i32;
    if f64::from(truncated) > value {
        truncated - 1
    } else {
        truncated
    }
}

fn ceil(value: f64) -> i32 {
    let truncated = value as i32;
    if f64::from(truncated) < value {
        truncated + 1
    } else {
        truncated
    }
}

fn outward<S>(rect: Rect<f64, S>) -> Rect<i32, S> {
    Rect::from_extremities(
        (floor(rect.loc.x), floor(rect.loc.y)),
        (
            ceil(rect.loc.x + rect.size.w),
            ceil(rect.loc.y + rect.size.h),
        ),
    )
}

fn ceil_div(value: i32, divisor: i32) -> i32 {
    -(-value).div_euclid(divisor)
}
