pub trait MathElement: Copy + 'static {
    const MANTISSA: u32;
    const BIAS: u64;
    const EXPONENT_MASK: u64;
    const MANTISSA_MASK: u64;
    const ONE_BITS: u64;
    const SUBNORMAL_SHIFT: u64;

    const ZERO: Self;
    const ONE: Self;
    const HALF: Self;
    const TWO: Self;
    const INFINITY: Self;
    const NAN: Self;
    const MIN_POSITIVE: Self;
    const SUBNORMAL_SCALE: Self;
    const MANTISSA_SCALE: Self;
    const ROUNDING_MAGIC: Self;
    const SPLITTER: Self;
    const SQRT_2: Self;

    const LN2: Self;
    const LN2_HIGH: Self;
    const LN2_LOW: Self;
    const LOG2_E: Self;
    const LOG2_E_HIGH: Self;
    const LOG2_E_LOW: Self;
    const LOG10_2: Self;
    const LOG10_E: Self;
    const EXP_LOW: Self;
    const EXP_HIGH: Self;
    const EXP2_LOW: Self;
    const EXP2_HIGH: Self;

    const TWO_OVER_PI: Self;
    const PI_2_PARTS: [Self; 4];
    const PI_HIGH: Self;
    const PI_LOW: Self;
    const PI_2_HIGH: Self;
    const PI_2_LOW: Self;
    const PI_4_HIGH: Self;
    const PI_4_LOW: Self;
    const TAN_PI_8: Self;
    const TAN_3PI_8: Self;

    const EXP_SERIES: &'static [Self];
    const LN_SERIES: &'static [Self];
    const SIN_SERIES: &'static [Self];
    const COS_SERIES: &'static [Self];
    const ATAN_SERIES: &'static [Self];
}

const PI_2_FIRST: f64 = f64::from_bits(0x3FF9_21FB_5440_0000);
const PI_2_SECOND: f64 = f64::from_bits(0x3DD0_B461_1A60_0000);
const PI_2_THIRD: f64 = f64::from_bits(0x3BA3_198A_2E00_0000);
const PI_2_FOURTH: f64 = f64::from_bits(0x397B_839A_2520_49C1);
const PI_2_TAIL: f64 = f64::from_bits(0x3DD0_B461_1A62_6331);
const LN2_TAIL: f64 = 2.319_046_813_846_299_6e-17;
const LOG2_E_TAIL: f64 = 2.035_527_374_093_103_3e-17;

const fn factorial(n: usize) -> f64 {
    let mut product = 1.0;
    let mut k = 2;
    while k <= n {
        product *= k as f64;
        k += 1;
    }
    product
}

const fn exp_series<const N: usize>() -> [f64; N] {
    let mut terms = [0.0; N];
    let mut index = 0;
    while index < N {
        terms[index] = 1.0 / factorial(N - 1 - index);
        index += 1;
    }
    terms
}

const fn odd_series<const N: usize>(first: usize, factorial_denominator: bool) -> [f64; N] {
    let mut terms = [0.0; N];
    let mut index = 0;
    while index < N {
        let power = first + 2 * (N - 1 - index);
        let magnitude = if factorial_denominator {
            1.0 / factorial(power)
        } else {
            1.0 / power as f64
        };
        let negative = ((power - first) / 2).is_multiple_of(2);
        terms[index] = if negative { -magnitude } else { magnitude };
        index += 1;
    }
    terms
}

const fn log_series<const N: usize>() -> [f64; N] {
    let mut terms = [0.0; N];
    let mut index = 0;
    while index < N {
        terms[index] = 2.0 / (3 + 2 * (N - 1 - index)) as f64;
        index += 1;
    }
    terms
}

const fn narrow<const N: usize>(values: [f64; N]) -> [f32; N] {
    let mut narrowed = [0.0; N];
    let mut index = 0;
    while index < N {
        narrowed[index] = values[index] as f32;
        index += 1;
    }
    narrowed
}

const fn keep_bits_f32(value: f32, significant: u32) -> f32 {
    f32::from_bits(value.to_bits() & !((1 << (24 - significant)) - 1))
}

const fn keep_bits_f64(value: f64, significant: u32) -> f64 {
    f64::from_bits(value.to_bits() & !((1 << (53 - significant)) - 1))
}

const fn pi_2_parts_f32() -> [f32; 4] {
    let mut parts = [0.0f32; 4];
    let mut rest = PI_2_FIRST + PI_2_TAIL;
    let mut tail = (PI_2_FIRST - rest) + PI_2_TAIL;
    let mut index = 0;
    while index < 3 {
        let part = keep_bits_f32(rest as f32, 8);
        parts[index] = part;
        let remainder = rest - part as f64;
        rest = remainder + tail;
        tail = (remainder - rest) + tail;
        index += 1;
    }
    parts[3] = rest as f32;
    parts
}

const fn split_f32(value: f64, tail: f64) -> (f32, f32) {
    let high = value as f32;
    (high, ((value - high as f64) + tail) as f32)
}

const LN2_F32: (f32, f32) = (
    keep_bits_f32(core::f64::consts::LN_2 as f32, 12),
    ((core::f64::consts::LN_2 - keep_bits_f32(core::f64::consts::LN_2 as f32, 12) as f64)
        + LN2_TAIL) as f32,
);
const LN2_F64_HIGH: f64 = keep_bits_f64(core::f64::consts::LN_2, 32);
const LN2_F64: (f64, f64) = (
    LN2_F64_HIGH,
    (core::f64::consts::LN_2 - LN2_F64_HIGH) + LN2_TAIL,
);
const LOG2_E_F32: (f32, f32) = split_f32(core::f64::consts::LOG2_E, LOG2_E_TAIL);
const PI_2_F32: (f32, f32) = split_f32(PI_2_FIRST, PI_2_TAIL);
const PI_F32: (f32, f32) = split_f32(2.0 * PI_2_FIRST, 2.0 * PI_2_TAIL);
const PI_4_F32: (f32, f32) = split_f32(0.5 * PI_2_FIRST, 0.5 * PI_2_TAIL);
const PI_2_F64_LOW: f64 = (PI_2_FIRST - core::f64::consts::FRAC_PI_2) + PI_2_TAIL;

const EXP_F32: [f32; 8] = narrow(exp_series::<8>());
const EXP_F64: [f64; 14] = exp_series::<14>();
const LN_F32: [f32; 4] = narrow(log_series::<4>());
const LN_F64: [f64; 10] = log_series::<10>();
const SIN_F32: [f32; 4] = narrow(odd_series::<4>(3, true));
const SIN_F64: [f64; 8] = odd_series::<8>(3, true);
const COS_F32: [f32; 5] = narrow(odd_series::<5>(2, true));
const COS_F64: [f64; 9] = odd_series::<9>(2, true);
const ATAN_F32: [f32; 9] = narrow(odd_series::<9>(3, false));
const ATAN_F64: [f64; 21] = odd_series::<21>(3, false);

impl MathElement for f32 {
    const MANTISSA: u32 = 23;
    const BIAS: u64 = 127;
    const EXPONENT_MASK: u64 = 0xFF;
    const MANTISSA_MASK: u64 = 0x007F_FFFF;
    const ONE_BITS: u64 = 0x3F80_0000;
    const SUBNORMAL_SHIFT: u64 = 25;

    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const HALF: Self = 0.5;
    const TWO: Self = 2.0;
    const INFINITY: Self = f32::INFINITY;
    const NAN: Self = f32::NAN;
    const MIN_POSITIVE: Self = f32::MIN_POSITIVE;
    const SUBNORMAL_SCALE: Self = 33_554_432.0;
    const MANTISSA_SCALE: Self = 8_388_608.0;
    const ROUNDING_MAGIC: Self = 12_582_912.0;
    const SPLITTER: Self = 4097.0;
    const SQRT_2: Self = core::f32::consts::SQRT_2;

    const LN2: Self = core::f32::consts::LN_2;
    const LN2_HIGH: Self = LN2_F32.0;
    const LN2_LOW: Self = LN2_F32.1;
    const LOG2_E: Self = core::f32::consts::LOG2_E;
    const LOG2_E_HIGH: Self = LOG2_E_F32.0;
    const LOG2_E_LOW: Self = LOG2_E_F32.1;
    const LOG10_2: Self = core::f32::consts::LOG10_2;
    const LOG10_E: Self = core::f32::consts::LOG10_E;
    const EXP_LOW: Self = -104.0;
    const EXP_HIGH: Self = 89.0;
    const EXP2_LOW: Self = -151.0;
    const EXP2_HIGH: Self = 129.0;

    const TWO_OVER_PI: Self = core::f32::consts::FRAC_2_PI;
    const PI_2_PARTS: [Self; 4] = pi_2_parts_f32();
    const PI_HIGH: Self = PI_F32.0;
    const PI_LOW: Self = PI_F32.1;
    const PI_2_HIGH: Self = PI_2_F32.0;
    const PI_2_LOW: Self = PI_2_F32.1;
    const PI_4_HIGH: Self = PI_4_F32.0;
    const PI_4_LOW: Self = PI_4_F32.1;
    const TAN_PI_8: Self = 0.414_213_57;
    const TAN_3PI_8: Self = 2.414_213_6;

    const EXP_SERIES: &'static [Self] = &EXP_F32;
    const LN_SERIES: &'static [Self] = &LN_F32;
    const SIN_SERIES: &'static [Self] = &SIN_F32;
    const COS_SERIES: &'static [Self] = &COS_F32;
    const ATAN_SERIES: &'static [Self] = &ATAN_F32;
}

impl MathElement for f64 {
    const MANTISSA: u32 = 52;
    const BIAS: u64 = 1023;
    const EXPONENT_MASK: u64 = 0x7FF;
    const MANTISSA_MASK: u64 = 0x000F_FFFF_FFFF_FFFF;
    const ONE_BITS: u64 = 0x3FF0_0000_0000_0000;
    const SUBNORMAL_SHIFT: u64 = 54;

    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const HALF: Self = 0.5;
    const TWO: Self = 2.0;
    const INFINITY: Self = f64::INFINITY;
    const NAN: Self = f64::NAN;
    const MIN_POSITIVE: Self = f64::MIN_POSITIVE;
    const SUBNORMAL_SCALE: Self = 18_014_398_509_481_984.0;
    const MANTISSA_SCALE: Self = 4_503_599_627_370_496.0;
    const ROUNDING_MAGIC: Self = 6_755_399_441_055_744.0;
    const SPLITTER: Self = 134_217_729.0;
    const SQRT_2: Self = core::f64::consts::SQRT_2;

    const LN2: Self = core::f64::consts::LN_2;
    const LN2_HIGH: Self = LN2_F64.0;
    const LN2_LOW: Self = LN2_F64.1;
    const LOG2_E: Self = core::f64::consts::LOG2_E;
    const LOG2_E_HIGH: Self = core::f64::consts::LOG2_E;
    const LOG2_E_LOW: Self = LOG2_E_TAIL;
    const LOG10_2: Self = core::f64::consts::LOG10_2;
    const LOG10_E: Self = core::f64::consts::LOG10_E;
    const EXP_LOW: Self = -746.0;
    const EXP_HIGH: Self = 710.0;
    const EXP2_LOW: Self = -1076.0;
    const EXP2_HIGH: Self = 1025.0;

    const TWO_OVER_PI: Self = core::f64::consts::FRAC_2_PI;
    const PI_2_PARTS: [Self; 4] = [PI_2_FIRST, PI_2_SECOND, PI_2_THIRD, PI_2_FOURTH];
    const PI_HIGH: Self = core::f64::consts::PI;
    const PI_LOW: Self = 2.0 * PI_2_F64_LOW;
    const PI_2_HIGH: Self = core::f64::consts::FRAC_PI_2;
    const PI_2_LOW: Self = PI_2_F64_LOW;
    const PI_4_HIGH: Self = core::f64::consts::FRAC_PI_4;
    const PI_4_LOW: Self = 0.5 * PI_2_F64_LOW;
    const TAN_PI_8: Self = 0.414_213_562_373_095_03;
    const TAN_3PI_8: Self = 2.414_213_562_373_095;

    const EXP_SERIES: &'static [Self] = &EXP_F64;
    const LN_SERIES: &'static [Self] = &LN_F64;
    const SIN_SERIES: &'static [Self] = &SIN_F64;
    const COS_SERIES: &'static [Self] = &COS_F64;
    const ATAN_SERIES: &'static [Self] = &ATAN_F64;
}
