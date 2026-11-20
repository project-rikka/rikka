use rikka_simd::prelude::*;

use crate::support::{Rng, Sample, lanes};

trait Reference: Sample + Copy {
    fn from_f64(value: f64) -> Self;
    fn to_f64(self) -> f64;
    fn ulps(a: Self, b: Self) -> u64;
    fn round(value: f64) -> Self;
}

impl Reference for f32 {
    fn from_f64(value: f64) -> Self {
        value as f32
    }

    fn to_f64(self) -> f64 {
        f64::from(self)
    }

    fn ulps(a: Self, b: Self) -> u64 {
        let ordered = |x: f32| {
            let bits = x.to_bits() as i32;
            i64::from(if bits < 0 { i32::MIN - bits } else { bits })
        };
        (ordered(a) - ordered(b)).unsigned_abs()
    }

    fn round(value: f64) -> Self {
        value as f32
    }
}

impl Reference for f64 {
    fn from_f64(value: f64) -> Self {
        value
    }

    fn to_f64(self) -> f64 {
        self
    }

    fn ulps(a: Self, b: Self) -> u64 {
        let ordered = |x: f64| {
            let bits = x.to_bits() as i64;
            i128::from(if bits < 0 { i64::MIN - bits } else { bits })
        };
        u64::try_from((ordered(a) - ordered(b)).unsigned_abs()).unwrap_or(u64::MAX)
    }

    fn round(value: f64) -> Self {
        value
    }
}

type Unary = (&'static str, fn(f64) -> f64);

struct Case {
    name: &'static str,
    ulps: u64,
    inputs: fn(&mut Rng) -> (f64, f64),
    reference: fn(f64, f64) -> f64,
}

fn uniform(rng: &mut Rng, low: f64, high: f64) -> f64 {
    let unit = (rng.next() >> 11) as f64 / (1u64 << 53) as f64;
    low + (high - low) * unit
}

fn log_uniform(rng: &mut Rng, low: f64, high: f64) -> f64 {
    uniform(rng, low.ln(), high.ln()).exp()
}

fn signed_log_uniform(rng: &mut Rng, low: f64, high: f64) -> f64 {
    let value = log_uniform(rng, low, high);
    if rng.next() & 1 == 0 { value } else { -value }
}

const CASES: &[Case] = &[
    Case {
        name: "exp",
        ulps: 2,
        inputs: |rng| (uniform(rng, -87.0, 88.0), 0.0),
        reference: |x, _| x.exp(),
    },
    Case {
        name: "exp2",
        ulps: 2,
        inputs: |rng| (uniform(rng, -125.0, 127.0), 0.0),
        reference: |x, _| x.exp2(),
    },
    Case {
        name: "ln",
        ulps: 2,
        inputs: |rng| (log_uniform(rng, 1e-37, 1e37), 0.0),
        reference: |x, _| x.ln(),
    },
    Case {
        name: "log2",
        ulps: 2,
        inputs: |rng| (log_uniform(rng, 1e-37, 1e37), 0.0),
        reference: |x, _| x.log2(),
    },
    Case {
        name: "log10",
        ulps: 2,
        inputs: |rng| (log_uniform(rng, 1e-37, 1e37), 0.0),
        reference: |x, _| x.log10(),
    },
    Case {
        name: "pow",
        ulps: 2,
        inputs: |rng| (log_uniform(rng, 1e-3, 1e3), uniform(rng, -12.0, 12.0)),
        reference: f64::powf,
    },
    Case {
        name: "sin",
        ulps: 2,
        inputs: |rng| (uniform(rng, -10_000.0, 10_000.0), 0.0),
        reference: |x, _| x.sin(),
    },
    Case {
        name: "cos",
        ulps: 2,
        inputs: |rng| (uniform(rng, -10_000.0, 10_000.0), 0.0),
        reference: |x, _| x.cos(),
    },
    Case {
        name: "tan",
        ulps: 3,
        inputs: |rng| (uniform(rng, -100.0, 100.0), 0.0),
        reference: |x, _| x.tan(),
    },
    Case {
        name: "asin",
        ulps: 3,
        inputs: |rng| (uniform(rng, -1.0, 1.0), 0.0),
        reference: |x, _| x.asin(),
    },
    Case {
        name: "acos",
        ulps: 3,
        inputs: |rng| (uniform(rng, -1.0, 1.0), 0.0),
        reference: |x, _| x.acos(),
    },
    Case {
        name: "atan",
        ulps: 2,
        inputs: |rng| (signed_log_uniform(rng, 1e-6, 1e6), 0.0),
        reference: |x, _| x.atan(),
    },
    Case {
        name: "atan2",
        ulps: 3,
        inputs: |rng| {
            (
                signed_log_uniform(rng, 1e-3, 1e3),
                signed_log_uniform(rng, 1e-3, 1e3),
            )
        },
        reference: f64::atan2,
    },
];

fn apply<V: FloatMath>(name: &str, x: V, y: V) -> V {
    match name {
        "exp" => x.exp(),
        "exp2" => x.exp2(),
        "ln" => x.ln(),
        "log2" => x.log2(),
        "log10" => x.log10(),
        "pow" => x.pow(y),
        "sin" => x.sin(),
        "cos" => x.cos(),
        "tan" => x.tan(),
        "asin" => x.asin(),
        "acos" => x.acos(),
        "atan" => x.atan(),
        "atan2" => x.atan2(y),
        _ => unreachable!("{name} is not a case"),
    }
}

fn accuracy<V: FloatMath>(isa: Isa, width: &str)
where
    V::Elem: Reference,
{
    for case in CASES {
        let mut rng = Rng::new(case.name.len() as u64 * 7919);
        let mut worst = (0, V::Elem::default(), V::Elem::default());
        for _ in 0..(512 / V::LANES).max(16) {
            let pairs: Vec<(V::Elem, V::Elem)> = (0..V::LANES)
                .map(|_| {
                    let (x, y) = (case.inputs)(&mut rng);
                    (V::Elem::from_f64(x), V::Elem::from_f64(y))
                })
                .collect();
            let x = V::from_fn(|lane| pairs[lane].0);
            let y = V::from_fn(|lane| pairs[lane].1);
            for (lane, value) in lanes(apply(case.name, x, y)).into_iter().enumerate() {
                let (a, b) = pairs[lane];
                let expected = V::Elem::round((case.reference)(a.to_f64(), b.to_f64()));
                let error = V::Elem::ulps(value, expected);
                if error > worst.0 {
                    worst = (error, a, b);
                }
            }
        }
        assert!(
            worst.0 <= case.ulps,
            "{isa} {width} {}: {} ulps at {:?}, {:?}",
            case.name,
            worst.0,
            worst.1,
            worst.2
        );
    }
}

fn specials<V: FloatMath>(isa: Isa, width: &str)
where
    V::Elem: Reference,
{
    let values = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        2.0,
        -2.0,
        0.5,
        -0.5,
        3.0,
        -3.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        1e-310,
        1e300,
    ];
    let unary: [Unary; 11] = [
        ("exp", f64::exp),
        ("exp2", f64::exp2),
        ("ln", f64::ln),
        ("log2", f64::log2),
        ("log10", f64::log10),
        ("sin", f64::sin),
        ("cos", f64::cos),
        ("tan", f64::tan),
        ("asin", f64::asin),
        ("acos", f64::acos),
        ("atan", f64::atan),
    ];
    for &x in &values {
        let element = V::Elem::from_f64(x);
        let vector = V::splat(element);
        for (name, reference) in unary {
            if matches!(name, "sin" | "cos" | "tan") && x.is_finite() && x.abs() > 1e9 {
                continue;
            }
            let expected = V::Elem::round(reference(element.to_f64()));
            let got = lanes(apply(name, vector, vector))[0];
            assert!(
                V::Elem::ulps(got, expected) <= 2
                    || (got.to_f64().is_nan() && expected.to_f64().is_nan()),
                "{isa} {width} {name}({x}) is {got:?}, expected {expected:?}"
            );
        }
        for &y in &values {
            let other = V::Elem::from_f64(y);
            for (name, reference) in [
                ("pow", f64::powf as fn(f64, f64) -> f64),
                ("atan2", f64::atan2),
            ] {
                let expected = V::Elem::round(reference(element.to_f64(), other.to_f64()));
                let got = lanes(apply(name, vector, V::splat(other)))[0];
                let both_nan = got.to_f64().is_nan() && expected.to_f64().is_nan();
                assert!(
                    both_nan
                        || (V::Elem::ulps(got, expected) <= 2
                            && got.to_f64().is_sign_negative()
                                == expected.to_f64().is_sign_negative()),
                    "{isa} {width} {name}({x}, {y}) is {got:?}, expected {expected:?}"
                );
            }
        }
    }
}

#[inline(always)]
fn functions<S: Simd>(_: S) {
    let isa = S::ISA;
    accuracy::<S::F32x1>(isa, "F32x1");
    accuracy::<S::F32x4>(isa, "F32x4");
    accuracy::<S::F32x16>(isa, "F32x16");
    accuracy::<S::F32s>(isa, "F32s");
    accuracy::<S::F64x1>(isa, "F64x1");
    accuracy::<S::F64x2>(isa, "F64x2");
    accuracy::<S::F64x8>(isa, "F64x8");
    accuracy::<S::F64s>(isa, "F64s");
    specials::<S::F32x4>(isa, "F32x4");
    specials::<S::F64s>(isa, "F64s");
}

#[test]
fn math_within_error_bounds() {
    for level in Level::supported() {
        rikka_simd::dispatch!(level, simd => functions(simd));
    }
}
