use std::cmp::Ordering;
use std::ops::{Add, Mul, Neg, Sub};

use smallvec::SmallVec;

type Limbs = SmallVec<[u64; 4]>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Int {
    negative: bool,
    limbs: Limbs,
}

impl Int {
    pub(crate) fn zero() -> Int {
        Int::default()
    }

    pub(crate) fn from_i64(value: i64) -> Int {
        let mut limbs = Limbs::new();
        if value != 0 {
            limbs.push(value.unsigned_abs());
        }
        Int {
            negative: value < 0,
            limbs,
        }
    }

    pub(crate) fn power_of_two(exponent: u32) -> Int {
        let mut limbs: Limbs = SmallVec::from_elem(0, exponent as usize / 64 + 1);
        limbs[exponent as usize / 64] = 1 << (exponent % 64);
        Int {
            negative: false,
            limbs,
        }
    }

    pub(crate) fn from_scaled_f64(value: f64, shift: u32) -> Int {
        if value == 0.0 {
            return Int::zero();
        }
        let bits = value.to_bits();
        let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1 << 52) - 1);
        let (mantissa, exponent) = match exponent_bits {
            0 => (fraction, -1074),
            _ => (fraction | (1 << 52), exponent_bits - 1075),
        };
        let total = exponent + shift as i32;
        let mut result = Int::from_i64(mantissa as i64);
        if total >= 0 {
            result = result.shl(total as u32);
        } else {
            result = Int::from_i64(mantissa.checked_shr((-total) as u32).unwrap_or(0) as i64);
        }
        result.negative = value < 0.0 && !result.is_zero();
        result
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub(crate) fn signum(&self) -> i32 {
        match (self.is_zero(), self.negative) {
            (true, _) => 0,
            (false, true) => -1,
            (false, false) => 1,
        }
    }

    pub(crate) fn is_negative(&self) -> bool {
        self.negative
    }

    pub(crate) fn negated(mut self) -> Int {
        if !self.is_zero() {
            self.negative = !self.negative;
        }
        self
    }

    pub(crate) fn shl(&self, bits: u32) -> Int {
        if self.is_zero() || bits == 0 {
            return self.clone();
        }
        let words = bits as usize / 64;
        let offset = bits % 64;
        let mut limbs: Limbs = SmallVec::from_elem(0, words);
        let mut carry = 0u64;
        for &limb in &self.limbs {
            match offset {
                0 => limbs.push(limb),
                _ => {
                    limbs.push((limb << offset) | carry);
                    carry = limb >> (64 - offset);
                }
            }
        }
        if carry != 0 {
            limbs.push(carry);
        }
        Int {
            negative: self.negative,
            limbs,
        }
    }

    pub(crate) fn magnitude_bits(&self) -> u64 {
        match self.limbs.last() {
            Some(top) => self.limbs.len() as u64 * 64 - u64::from(top.leading_zeros()),
            None => 0,
        }
    }

    pub(crate) fn approximate(&self) -> (f64, i64) {
        let bits = self.magnitude_bits();
        if bits == 0 {
            return (0.0, 0);
        }
        let shift = bits.saturating_sub(64);
        let word = (shift / 64) as usize;
        let offset = (shift % 64) as u32;
        let low = self.limbs[word] as u128;
        let high = self.limbs.get(word + 1).copied().unwrap_or(0) as u128;
        let combined = (low | (high << 64)) >> offset;
        let sticky = self.limbs[..word].iter().any(|&limb| limb != 0)
            || (offset > 0 && (self.limbs[word] & ((1u64 << offset) - 1)) != 0);
        let mut top = combined as u64;
        if sticky {
            top |= 1;
        }
        let value = top as f64;
        (
            match self.negative {
                true => -value,
                false => value,
            },
            shift as i64,
        )
    }

    pub(crate) fn mul_i64(&self, factor: i64) -> Int {
        self * &Int::from_i64(factor)
    }
}

pub(crate) fn ldexp(value: f64, exponent: i64) -> f64 {
    let mut result = value;
    let mut remaining = exponent;
    while remaining > 1000 {
        result *= f64::from_bits(((1023 + 1000) as u64) << 52);
        remaining -= 1000;
    }
    while remaining < -1000 {
        result *= f64::from_bits(((1023 - 1000) as u64) << 52);
        remaining += 1000;
    }
    result * f64::from_bits(((1023 + remaining) as u64) << 52)
}

pub(crate) fn ratio(numerator: &Int, denominator: &Int, shift: u32) -> f64 {
    let (top, top_exponent) = numerator.approximate();
    let (bottom, bottom_exponent) = denominator.approximate();
    if top == 0.0 {
        return 0.0;
    }
    ldexp(
        top / bottom,
        top_exponent - bottom_exponent - i64::from(shift),
    )
}

fn compare_magnitude(a: &[u64], b: &[u64]) -> Ordering {
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    for (x, y) in a.iter().rev().zip(b.iter().rev()) {
        if x != y {
            return x.cmp(y);
        }
    }
    Ordering::Equal
}

fn add_magnitude(a: &[u64], b: &[u64]) -> Limbs {
    let (long, short) = match a.len() >= b.len() {
        true => (a, b),
        false => (b, a),
    };
    let mut result = Limbs::with_capacity(long.len() + 1);
    let mut carry = 0u64;
    for (index, &limb) in long.iter().enumerate() {
        let other = short.get(index).copied().unwrap_or(0);
        let (sum, first) = limb.overflowing_add(other);
        let (sum, second) = sum.overflowing_add(carry);
        result.push(sum);
        carry = u64::from(first) + u64::from(second);
    }
    if carry != 0 {
        result.push(carry);
    }
    result
}

fn sub_magnitude(a: &[u64], b: &[u64]) -> Limbs {
    let mut result = Limbs::with_capacity(a.len());
    let mut borrow = 0u64;
    for (index, &limb) in a.iter().enumerate() {
        let other = b.get(index).copied().unwrap_or(0);
        let (difference, first) = limb.overflowing_sub(other);
        let (difference, second) = difference.overflowing_sub(borrow);
        result.push(difference);
        borrow = u64::from(first) + u64::from(second);
    }
    while result.last() == Some(&0) {
        result.pop();
    }
    result
}

fn signed_sum(a_negative: bool, a: &[u64], b_negative: bool, b: &[u64]) -> Int {
    if a_negative == b_negative {
        let limbs = add_magnitude(a, b);
        return Int {
            negative: a_negative && !limbs.is_empty(),
            limbs,
        };
    }
    match compare_magnitude(a, b) {
        Ordering::Equal => Int::zero(),
        Ordering::Greater => Int {
            negative: a_negative,
            limbs: sub_magnitude(a, b),
        },
        Ordering::Less => Int {
            negative: b_negative,
            limbs: sub_magnitude(b, a),
        },
    }
}

impl Add for &Int {
    type Output = Int;

    fn add(self, other: &Int) -> Int {
        signed_sum(self.negative, &self.limbs, other.negative, &other.limbs)
    }
}

impl Sub for &Int {
    type Output = Int;

    fn sub(self, other: &Int) -> Int {
        signed_sum(
            self.negative,
            &self.limbs,
            !other.negative && !other.is_zero(),
            &other.limbs,
        )
    }
}

impl Mul for &Int {
    type Output = Int;

    fn mul(self, other: &Int) -> Int {
        if self.is_zero() || other.is_zero() {
            return Int::zero();
        }
        let mut limbs: Limbs = SmallVec::from_elem(0, self.limbs.len() + other.limbs.len());
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u128;
            for (j, &b) in other.limbs.iter().enumerate() {
                let product = u128::from(a) * u128::from(b) + u128::from(limbs[i + j]) + carry;
                limbs[i + j] = product as u64;
                carry = product >> 64;
            }
            let mut index = i + other.limbs.len();
            while carry != 0 {
                let sum = u128::from(limbs[index]) + carry;
                limbs[index] = sum as u64;
                carry = sum >> 64;
                index += 1;
            }
        }
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        Int {
            negative: self.negative != other.negative,
            limbs,
        }
    }
}

impl Neg for Int {
    type Output = Int;

    fn neg(self) -> Int {
        self.negated()
    }
}

impl Add for Int {
    type Output = Int;

    fn add(self, other: Int) -> Int {
        &self + &other
    }
}

impl Sub for Int {
    type Output = Int;

    fn sub(self, other: Int) -> Int {
        &self - &other
    }
}

impl Mul for Int {
    type Output = Int;

    fn mul(self, other: Int) -> Int {
        &self * &other
    }
}

impl PartialOrd for Int {
    fn partial_cmp(&self, other: &Int) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Int {
    fn cmp(&self, other: &Int) -> Ordering {
        match (self.signum(), other.signum()) {
            (a, b) if a != b => a.cmp(&b),
            (0, _) => Ordering::Equal,
            (1, _) => compare_magnitude(&self.limbs, &other.limbs),
            _ => compare_magnitude(&other.limbs, &self.limbs),
        }
    }
}

pub(crate) fn det2(a: &Int, b: &Int, c: &Int, d: &Int) -> Int {
    &(a * d) - &(b * c)
}

pub(crate) fn det3(rows: [[&Int; 3]; 3]) -> Int {
    let [a, b, c] = rows;
    let first = a[0] * &det2(b[1], b[2], c[1], c[2]);
    let second = a[1] * &det2(b[0], b[2], c[0], c[2]);
    let third = a[2] * &det2(b[0], b[1], c[0], c[1]);
    &(&first - &second) + &third
}

pub(crate) fn det4(rows: &[[Int; 4]; 4]) -> Int {
    let pairs = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    let top: Vec<Int> = pairs
        .iter()
        .map(|&(i, j)| det2(&rows[0][i], &rows[0][j], &rows[1][i], &rows[1][j]))
        .collect();
    let bottom: Vec<Int> = pairs
        .iter()
        .map(|&(i, j)| det2(&rows[2][i], &rows[2][j], &rows[3][i], &rows[3][j]))
        .collect();
    let mut total = Int::zero();
    for (index, &(i, j)) in pairs.iter().enumerate() {
        let complement = pairs
            .iter()
            .position(|&(k, l)| k != i && k != j && l != i && l != j)
            .unwrap_or(0);
        let sign_index = i + j + 1;
        let term = &top[index] * &bottom[complement];
        total = match sign_index % 2 == 0 {
            true => &total + &term,
            false => &total - &term,
        };
    }
    total
}
