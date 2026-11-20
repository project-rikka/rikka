const OPAQUE_FROM: u8 = 128;
const MAX_COLORS: usize = 256;
const BIN_BITS: u32 = 5;
const BINS: usize = 1 << (BIN_BITS * 3);
const SEEN_SLOTS: usize = 1024;
const SEEN_SHIFT: u32 = 22;
const SEEN_MULTIPLIER: u32 = 0x9E37_79B1;
const UNSEEN: u32 = u32::MAX;

#[derive(Clone, Debug, Default)]
pub(crate) struct Indexed {
    pub(crate) palette: Vec<[u8; 3]>,
    pub(crate) indices: Vec<u8>,
    pub(crate) transparent: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Bin {
    count: u64,
    sums: [u64; 3],
}

impl Bin {
    fn mean(&self, channel: usize) -> f64 {
        self.sums[channel] as f64 / self.count as f64
    }
}

struct Cut {
    start: usize,
    end: usize,
    channel: usize,
    spread: f64,
}

#[derive(Debug)]
pub(crate) struct Quantizer {
    bins: Vec<Bin>,
    order: Vec<u16>,
    assigned: Vec<u8>,
    seen: Vec<(u32, u8)>,
}

#[inline]
fn bin_of(pixel: &[u8; 4]) -> usize {
    let part = |channel: usize| usize::from(pixel[channel] >> (8 - BIN_BITS));
    part(0) << (BIN_BITS * 2) | part(1) << BIN_BITS | part(2)
}

impl Quantizer {
    pub(crate) fn new() -> Self {
        Self {
            bins: vec![Bin::default(); BINS],
            order: Vec::new(),
            assigned: vec![0; BINS],
            seen: vec![(UNSEEN, 0); SEEN_SLOTS],
        }
    }

    pub(crate) fn quantize(&mut self, rgba: &[u8], indexed: &mut Indexed) {
        let pixels = rgba.as_chunks::<4>().0;
        indexed.palette.clear();
        indexed.indices.clear();
        indexed.indices.reserve(pixels.len());
        let transparent = pixels.iter().any(|pixel| pixel[3] < OPAQUE_FROM);
        let room = MAX_COLORS - usize::from(transparent);
        if !self.exact(pixels, room, indexed) {
            self.reduced(pixels, room, indexed);
        }
        indexed.transparent = transparent.then(|| {
            indexed.palette.push([0; 3]);
            (indexed.palette.len() - 1) as u8
        });
        if let Some(index) = indexed.transparent {
            for (target, pixel) in indexed.indices.iter_mut().zip(pixels) {
                if pixel[3] < OPAQUE_FROM {
                    *target = index;
                }
            }
        }
    }

    fn exact(&mut self, pixels: &[[u8; 4]], room: usize, indexed: &mut Indexed) -> bool {
        self.seen.fill((UNSEEN, 0));
        for pixel in pixels {
            if pixel[3] < OPAQUE_FROM {
                indexed.indices.push(0);
                continue;
            }
            let color = u32::from_le_bytes([pixel[0], pixel[1], pixel[2], 0]);
            let mut slot = (color.wrapping_mul(SEEN_MULTIPLIER) >> SEEN_SHIFT) as usize;
            while self.seen[slot].0 != UNSEEN && self.seen[slot].0 != color {
                slot = (slot + 1) % SEEN_SLOTS;
            }
            if self.seen[slot].0 == UNSEEN {
                if indexed.palette.len() == room {
                    indexed.palette.clear();
                    indexed.indices.clear();
                    return false;
                }
                self.seen[slot] = (color, indexed.palette.len() as u8);
                indexed.palette.push([pixel[0], pixel[1], pixel[2]]);
            }
            indexed.indices.push(self.seen[slot].1);
        }
        true
    }

    fn reduced(&mut self, pixels: &[[u8; 4]], room: usize, indexed: &mut Indexed) {
        self.bins.fill(Bin::default());
        for pixel in pixels.iter().filter(|pixel| pixel[3] >= OPAQUE_FROM) {
            let bin = &mut self.bins[bin_of(pixel)];
            bin.count += 1;
            for channel in 0..3 {
                bin.sums[channel] += u64::from(pixel[channel]);
            }
        }
        self.order.clear();
        self.order.extend(
            (0..BINS)
                .filter(|&bin| self.bins[bin].count > 0)
                .map(|bin| bin as u16),
        );
        let mut cuts = vec![self.measure(0, self.order.len())];
        while cuts.len() < room {
            let Some((widest, _)) = cuts
                .iter()
                .enumerate()
                .filter(|(_, cut)| cut.end - cut.start > 1)
                .max_by(|(_, left), (_, right)| left.spread.total_cmp(&right.spread))
            else {
                break;
            };
            let Cut {
                start,
                end,
                channel,
                ..
            } = cuts[widest];
            let bins = &self.bins;
            self.order[start..end].sort_unstable_by(|&left, &right| {
                bins[usize::from(left)]
                    .mean(channel)
                    .total_cmp(&bins[usize::from(right)].mean(channel))
            });
            let total: u64 = self.order[start..end]
                .iter()
                .map(|&bin| bins[usize::from(bin)].count)
                .sum();
            let mut below = 0;
            let mut middle = start + 1;
            for (offset, &bin) in self.order[start..end - 1].iter().enumerate() {
                below += bins[usize::from(bin)].count;
                middle = start + offset + 1;
                if below * 2 >= total {
                    break;
                }
            }
            cuts[widest] = self.measure(start, middle);
            cuts.push(self.measure(middle, end));
        }
        for (index, cut) in cuts.iter().enumerate() {
            let mut whole = Bin::default();
            for &bin in &self.order[cut.start..cut.end] {
                let bin = usize::from(bin);
                whole.count += self.bins[bin].count;
                for channel in 0..3 {
                    whole.sums[channel] += self.bins[bin].sums[channel];
                }
                self.assigned[bin] = index as u8;
            }
            indexed
                .palette
                .push([0, 1, 2].map(|channel| whole.mean(channel).round() as u8));
        }
        indexed
            .indices
            .extend(pixels.iter().map(|pixel| self.assigned[bin_of(pixel)]));
    }

    fn measure(&self, start: usize, end: usize) -> Cut {
        let mut whole = Bin::default();
        let mut squares = [0f64; 3];
        for &bin in &self.order[start..end] {
            let bin = &self.bins[usize::from(bin)];
            whole.count += bin.count;
            for channel in 0..3 {
                whole.sums[channel] += bin.sums[channel];
                squares[channel] += bin.mean(channel) * bin.sums[channel] as f64;
            }
        }
        let (channel, spread) = (0..3)
            .map(|channel| {
                let mean = whole.mean(channel);
                (
                    channel,
                    squares[channel] - mean * whole.sums[channel] as f64,
                )
            })
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .unwrap_or((0, 0.0));
        Cut {
            start,
            end,
            channel,
            spread,
        }
    }
}
