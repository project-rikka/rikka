use crate::cff::active_regions::ActiveRegions;
use crate::parse::f2dot14::F2Dot14;
use crate::variations::item_variation_store::ItemVariationStore;

pub(crate) const MAX_REGIONS: usize = 512;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Blend<'a, 'c> {
    pub(crate) store: ItemVariationStore<'a>,
    pub(crate) coords: &'c [F2Dot14],
}

impl Blend<'_, '_> {
    pub(crate) fn active_regions(&self, vsindex: u16, out: &mut ActiveRegions) -> Option<()> {
        let data = self.store.variation_data(vsindex)?;
        let indices = data.region_indices();
        if indices.len() > MAX_REGIONS {
            return None;
        }
        let regions = self.store.regions();
        out.count = indices.len();
        out.active.clear();
        for (slot, region) in indices.iter().enumerate() {
            let scalar = regions.scalar(usize::from(region), self.coords);
            if scalar != 0.0 {
                out.active.push((slot, f64::from(scalar)));
            }
        }
        Some(())
    }
}

pub(crate) fn apply_blend(stack: &mut [f64], len: usize, regions: &ActiveRegions) -> Option<usize> {
    let count = *stack.get(len.checked_sub(1)?)?;
    if !(0.0..=f64::from(u16::MAX)).contains(&count) {
        return None;
    }
    let count = count as usize;
    let region_count = regions.count;
    let operands = count.checked_mul(region_count + 1)?;
    let base = (len - 1).checked_sub(operands)?;
    if regions.active.is_empty() || count == 0 {
        return Some(base + count);
    }
    let (values, deltas) = stack[base..len - 1].split_at_mut(count);
    for (value, deltas) in values.iter_mut().zip(deltas.chunks_exact(region_count)) {
        let mut sum = *value;
        for &(slot, scalar) in &regions.active {
            sum += deltas[slot] * scalar;
        }
        *value = sum;
    }
    Some(base + count)
}
