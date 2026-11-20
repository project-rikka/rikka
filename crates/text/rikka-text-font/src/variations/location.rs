use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::font::Font;
use crate::sfnt::tag::Tag;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Location {
    coords: Box<[F2Dot14]>,
}

impl Location {
    pub fn from_normalized(coords: impl Into<Box<[F2Dot14]>>) -> Self {
        let coords = coords.into();
        if coords.iter().all(|c| c.0 == 0) {
            return Self::default();
        }
        Self { coords }
    }

    #[inline]
    pub fn coords(&self) -> &[F2Dot14] {
        &self.coords
    }

    #[inline]
    pub fn coord(&self, axis: usize) -> F2Dot14 {
        self.coords.get(axis).copied().unwrap_or_default()
    }

    #[inline]
    pub fn is_default(&self) -> bool {
        self.coords.is_empty()
    }
}

impl Font<'_> {
    pub fn location(&self, settings: impl IntoIterator<Item = (Tag, f32)>) -> Location {
        let Some(fvar) = self.fvar() else {
            return Location::default();
        };
        let axes: Vec<_> = fvar.axes().collect();
        let mut values: Vec<f32> = axes.iter().map(|axis| axis.default).collect();
        for (tag, value) in settings {
            for (axis, slot) in axes.iter().zip(values.iter_mut()) {
                if axis.tag == tag {
                    *slot = value;
                }
            }
        }
        self.location_from_user(&values)
    }

    pub fn named_instance_location(&self, index: u16) -> Option<Location> {
        let instance = self.fvar()?.instance(index)?;
        let values: Vec<f32> = instance.coordinates().collect();
        Some(self.location_from_user(&values))
    }

    fn location_from_user(&self, values: &[f32]) -> Location {
        let Some(fvar) = self.fvar() else {
            return Location::default();
        };
        let mut coords: Vec<F2Dot14> = fvar
            .axes()
            .zip(values)
            .map(|(axis, &value)| F2Dot14::from_f32(axis.normalize(value)))
            .collect();
        if let Some(avar) = self.avar() {
            avar.map(&mut coords);
        }
        Location::from_normalized(coords)
    }
}
