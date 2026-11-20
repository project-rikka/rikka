use crate::cff::blend::Blend;
use crate::parse::f2dot14::F2Dot14;

#[derive(Debug, Default)]
pub(crate) struct ActiveRegions {
    pub(crate) count: usize,
    pub(crate) active: Vec<(usize, f64)>,
    key: Option<u16>,
    coords: Vec<F2Dot14>,
}

impl ActiveRegions {
    pub(crate) fn prepare(&mut self, blend: Option<Blend<'_, '_>>, vsindex: u16) -> Option<()> {
        let Some(blend) = blend else {
            self.key = None;
            self.count = 0;
            self.active.clear();
            return Some(());
        };
        if self.key == Some(vsindex) && self.coords == blend.coords {
            return Some(());
        }
        self.key = None;
        blend.active_regions(vsindex, self)?;
        self.coords.clear();
        self.coords.extend_from_slice(blend.coords);
        self.key = Some(vsindex);
        Some(())
    }

    pub(crate) fn invalidate(&mut self) {
        self.key = None;
    }
}
