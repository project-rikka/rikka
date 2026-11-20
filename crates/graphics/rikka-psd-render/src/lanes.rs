use rikka_simd::{Float, Gather, Simd, Vector};

const MAX_LANES: usize = 16;

pub(crate) trait Lanes: Float<Elem = f32> + Gather<Index = <Self as Float>::Bits> {}

impl<V: Float<Elem = f32> + Gather<Index = <V as Float>::Bits>> Lanes for V {}

pub(crate) trait Kernel {
    fn run<V: Lanes>(&mut self, at: usize);
}

macro_rules! by_channels {
    ($channels:expr, $c:ident => $body:expr) => {
        match $channels {
            1 => {
                const $c: usize = 1;
                $body
            }
            3 => {
                const $c: usize = 3;
                $body
            }
            4 => {
                const $c: usize = 4;
                $body
            }
            channels => panic!("unsupported raster channel count {channels}"),
        }
    };
}

pub(crate) use by_channels;

#[inline(always)]
pub(crate) fn sweep<S: Simd, K: Kernel>(_: S, kernel: &mut K, count: usize) {
    let lanes = S::F32s::LANES;
    let whole = count / lanes * lanes;
    for at in (0..whole).step_by(lanes) {
        kernel.run::<S::F32s>(at);
    }
    for at in whole..count {
        kernel.run::<S::F32x1>(at);
    }
}

pub(crate) struct Pixels<V, const C: usize> {
    pub(crate) color: [V; C],
    pub(crate) alpha: V,
}

impl<V: Lanes, const C: usize> Pixels<V, C> {
    #[inline(always)]
    pub(crate) fn load(row: &[f32], at: usize) -> Self {
        let stride = C + 1;
        let values = &row[at * stride..(at + V::LANES) * stride];
        let mut color = [V::default(); C];
        if stride == 4 {
            let (pixels, _) = values.as_chunks::<4>();
            let mut rows = [&[0.0f32; 4]; MAX_LANES];
            for (row, pixel) in rows.iter_mut().zip(pixels) {
                *row = pixel;
            }
            let columns = V::load_transposed(&rows[..V::LANES]);
            color.copy_from_slice(&columns[..C]);
            return Self {
                color,
                alpha: columns[3],
            };
        }
        for (channel, slot) in color.iter_mut().enumerate() {
            *slot = V::from_fn(|lane| values[lane * stride + channel]);
        }
        Self {
            color,
            alpha: V::from_fn(|lane| values[lane * stride + C]),
        }
    }

    #[inline(always)]
    pub(crate) fn store(self, row: &mut [f32], at: usize) {
        let stride = C + 1;
        let values = &mut row[at * stride..(at + V::LANES) * stride];
        if stride == 4 {
            let mut columns = [self.alpha; 4];
            columns[..C].copy_from_slice(&self.color);
            let (pixels, _) = values.as_chunks_mut::<4>();
            let mut spare = [[0.0f32; 4]; MAX_LANES];
            let mut rows = spare.each_mut();
            for (slot, pixel) in rows.iter_mut().zip(pixels) {
                *slot = pixel;
            }
            V::store_transposed(columns, &mut rows[..V::LANES]);
            return;
        }
        let mut lanes = [0.0f32; MAX_LANES];
        for (channel, vector) in self.color.into_iter().chain([self.alpha]).enumerate() {
            vector.store(&mut lanes[..V::LANES]);
            for (lane, value) in lanes[..V::LANES].iter().enumerate() {
                values[lane * stride + channel] = *value;
            }
        }
    }
}
