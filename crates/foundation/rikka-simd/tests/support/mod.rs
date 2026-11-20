use std::fmt::Debug;

use rikka_simd::{Element, Vector};

pub(crate) const WINDOWS: usize = 24;

pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub(crate) fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

pub(crate) trait Sample: Element + Debug {
    fn samples() -> Vec<Self>;
    fn same(a: Self, b: Self) -> bool;
}

macro_rules! int_samples {
    ($($element:ty),*) => {
        $(
            impl Sample for $element {
                fn samples() -> Vec<Self> {
                    let mut values = vec![
                        0, 1, 2, 3, 7, 8, 15, 16, 31, 63, 64,
                        <$element>::MAX, <$element>::MIN,
                        <$element>::MAX - 1, <$element>::MIN.wrapping_add(1),
                        <$element>::MAX / 2, <$element>::MIN / 2,
                        (0 as $element).wrapping_sub(1), (0 as $element).wrapping_sub(5),
                    ];
                    let mut rng = Rng::new(size_of::<$element>() as u64);
                    values.extend((0..109).map(|index| {
                        let value = rng.next() as $element;
                        if index % 3 == 0 { value >> (index % (8 * size_of::<$element>())) } else { value }
                    }));
                    values
                }

                fn same(a: Self, b: Self) -> bool {
                    a == b
                }
            }
        )*
    };
}

int_samples!(i8, u8, i16, u16, i32, u32, i64, u64);

macro_rules! float_samples {
    ($($element:ty),*) => {
        $(
            impl Sample for $element {
                fn samples() -> Vec<Self> {
                    let mut values = vec![
                        0.0, -0.0, 0.5, -0.5, 1.0, -1.0, 1.5, -1.5, 2.5, -2.5, 3.5,
                        0.49999997, -0.49999997, 1e10, -1e10, 1e-10, -3e-7, 123.456, -987.25,
                        <$element>::INFINITY, <$element>::NEG_INFINITY, <$element>::NAN,
                        <$element>::MAX, <$element>::MIN, <$element>::MIN_POSITIVE,
                        <$element>::EPSILON, <$element>::MIN_POSITIVE / 4.0,
                        8_388_607.5, -8_388_608.5, 4_503_599_627_370_495.5, 2_147_483_647.0,
                        -2_147_483_648.0, 2_147_483_648.0, 9.3e18, -9.3e18,
                    ];
                    let mut rng = Rng::new(size_of::<$element>() as u64 + 99);
                    values.extend((0..93).map(|index| {
                        let unit = (rng.next() >> 11) as $element / (1u64 << 53) as $element;
                        let scale = [1.0, 10.0, 1000.0, 1e6, 0.001][index % 5];
                        (unit * 2.0 - 1.0) * scale
                    }));
                    values
                }

                fn same(a: Self, b: Self) -> bool {
                    (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
                }
            }
        )*
    };
}

float_samples!(f32, f64);

pub(crate) fn window<V: Vector>(samples: &[V::Elem], start: usize) -> V
where
    V::Elem: Sample,
{
    V::from_fn(|lane| samples[(start + lane * 3) % samples.len()])
}

pub(crate) fn lanes<V: Vector>(vector: V) -> Vec<V::Elem> {
    let mut lanes = vec![V::Elem::default(); V::LANES];
    vector.store(&mut lanes);
    lanes
}

pub(crate) fn assert_lanes<E: Sample>(context: &str, got: &[E], expected: &[E]) {
    assert_eq!(got.len(), expected.len(), "{context}: lane count");
    for (lane, (&value, &reference)) in got.iter().zip(expected).enumerate() {
        assert!(
            E::same(value, reference),
            "{context}: lane {lane} is {value:?}, expected {reference:?} in {got:?}"
        );
    }
}

macro_rules! each_level {
    ($kernel:ident) => {
        for level in rikka_simd::Level::supported() {
            rikka_simd::dispatch!(level, simd => $kernel(simd));
        }
    };
}

pub(crate) use each_level;
