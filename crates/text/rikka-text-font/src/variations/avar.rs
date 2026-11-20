use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;
use crate::variations::delta_set_index_map::DeltaSetIndexMap;
use crate::variations::item_variation_store::{DeltaSetIndex, ItemVariationStore};

#[derive(Clone, Copy, Debug)]
pub struct Avar<'a> {
    data: &'a [u8],
    axis_count: u16,
    maps_end: usize,
    version: u16,
}

impl<'a> Avar<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        let version = data.u16_at(0)?;
        if version != 1 && version != 2 {
            return None;
        }
        let axis_count = data.u16_at(6)?;
        let mut offset = 8;
        for _ in 0..axis_count {
            let pairs = usize::from(data.u16_at(offset)?);
            offset += 2 + pairs * 4;
        }
        data.range(0, offset)?;
        Some(Self {
            data,
            axis_count,
            maps_end: offset,
            version,
        })
    }

    pub fn map(&self, coords: &mut [F2Dot14]) {
        self.map_segments(coords);
        if self.version == 2 {
            self.map_variations(coords);
        }
    }

    fn map_segments(&self, coords: &mut [F2Dot14]) {
        let mut offset = 8;
        for axis in 0..usize::from(self.axis_count) {
            let Some(pairs) = self.data.u16_at(offset) else {
                return;
            };
            let map = self
                .data
                .range(offset + 2, usize::from(pairs) * 4)
                .unwrap_or_default();
            if let Some(coord) = coords.get_mut(axis) {
                *coord = map_segment(map, *coord);
            }
            offset += 2 + usize::from(pairs) * 4;
        }
    }

    fn map_variations(&self, coords: &mut [F2Dot14]) {
        let index_map = self
            .data
            .offset32(0, self.maps_end)
            .and_then(DeltaSetIndexMap::new);
        let Some(store) = self
            .data
            .offset32(0, self.maps_end + 4)
            .and_then(ItemVariationStore::new)
        else {
            return;
        };
        let input: Vec<F2Dot14> = coords.to_vec();
        for (axis, coord) in coords.iter_mut().enumerate() {
            let index = match index_map {
                Some(map) => map.get(axis as u32),
                None => Some(DeltaSetIndex {
                    outer: 0,
                    inner: axis as u16,
                }),
            };
            let Some(index) = index else { continue };
            if index.outer == 0xFFFF && index.inner == 0xFFFF {
                continue;
            }
            let delta = store.delta(index, &input);
            let mapped = f32::from(coord.0) + delta.round();
            *coord = F2Dot14(mapped.clamp(-16384.0, 16384.0) as i16);
        }
    }
}

fn map_segment(map: &[u8], coord: F2Dot14) -> F2Dot14 {
    let pair = |index: usize| -> (i32, i32) {
        (
            i32::from(map.i16_at(index * 4).unwrap_or(0)),
            i32::from(map.i16_at(index * 4 + 2).unwrap_or(0)),
        )
    };
    let count = map.len() / 4;
    let value = i32::from(coord.0);
    let mapped = match count {
        0 => value,
        1 => {
            let (from, to) = pair(0);
            value - from + to
        }
        _ => {
            let (first_from, first_to) = pair(0);
            let (last_from, last_to) = pair(count - 1);
            if value <= first_from {
                value - first_from + first_to
            } else if value >= last_from {
                value - last_from + last_to
            } else {
                let mut upper = 1;
                while upper < count - 1 && pair(upper).0 < value {
                    upper += 1;
                }
                let (to_from, to_to) = pair(upper);
                if value == to_from {
                    to_to
                } else {
                    let (from_from, from_to) = pair(upper - 1);
                    let denominator = to_from - from_from;
                    if denominator == 0 {
                        from_to
                    } else {
                        (from_to as f32
                            + (to_to - from_to) as f32 * (value - from_from) as f32
                                / denominator as f32)
                            .round() as i32
                    }
                }
            }
        }
    };
    F2Dot14(mapped.clamp(-16384, 16384) as i16)
}

impl<'a> Font<'a> {
    pub fn avar(&self) -> Option<Avar<'a>> {
        Avar::new(self.table_data(TableId::Avar)?)
    }
}
