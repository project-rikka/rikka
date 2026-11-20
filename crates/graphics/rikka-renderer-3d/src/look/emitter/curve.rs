use glam::Vec3;

pub const CURVE_KEYS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    ages: [f32; CURVE_KEYS],
    values: [f32; CURVE_KEYS],
}

impl Curve {
    pub fn constant(value: f32) -> Self {
        Self::keyed([(0.0, value)])
    }

    pub fn linear(from: f32, to: f32) -> Self {
        Self::keyed([(0.0, from), (1.0, to)])
    }

    pub fn keyed<const N: usize>(keys: [(f32, f32); N]) -> Self {
        const { assert!(N >= 1 && N <= CURVE_KEYS, "a curve holds one to four keys") };
        let (ages, values) = padded(keys);
        Self { ages, values }
    }

    pub fn scaled(self, factor: f32) -> Self {
        Self {
            ages: self.ages,
            values: self.values.map(|value| value * factor),
        }
    }

    pub fn sample(&self, age: f32) -> f32 {
        let (at, mix) = locate(&self.ages, age);
        let to = (at + 1).min(CURVE_KEYS - 1);
        self.values[at] + (self.values[to] - self.values[at]) * mix
    }

    pub(crate) fn ages(&self) -> [f32; CURVE_KEYS] {
        self.ages
    }

    pub(crate) fn values(&self) -> [f32; CURVE_KEYS] {
        self.values
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    ages: [f32; CURVE_KEYS],
    colors: [Vec3; CURVE_KEYS],
}

impl Gradient {
    pub fn constant(color: Vec3) -> Self {
        Self::keyed([(0.0, color)])
    }

    pub fn linear(from: Vec3, to: Vec3) -> Self {
        Self::keyed([(0.0, from), (1.0, to)])
    }

    pub fn keyed<const N: usize>(stops: [(f32, Vec3); N]) -> Self {
        const {
            assert!(
                N >= 1 && N <= CURVE_KEYS,
                "a gradient holds one to four stops"
            )
        };
        let (ages, colors) = padded(stops);
        Self { ages, colors }
    }

    pub fn tinted(self, tint: Vec3) -> Self {
        Self {
            ages: self.ages,
            colors: self.colors.map(|color| color * tint),
        }
    }

    pub fn sample(&self, age: f32) -> Vec3 {
        let (at, mix) = locate(&self.ages, age);
        let to = (at + 1).min(CURVE_KEYS - 1);
        self.colors[at] + (self.colors[to] - self.colors[at]) * mix
    }

    pub(crate) fn ages(&self) -> [f32; CURVE_KEYS] {
        self.ages
    }

    pub(crate) fn colors(&self) -> [Vec3; CURVE_KEYS] {
        self.colors
    }
}

fn padded<T: Copy, const N: usize>(
    mut keys: [(f32, T); N],
) -> ([f32; CURVE_KEYS], [T; CURVE_KEYS]) {
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    let last = keys[N - 1];
    let key = |index: usize| keys.get(index).copied().unwrap_or(last);
    (
        std::array::from_fn(|index| key(index).0),
        std::array::from_fn(|index| key(index).1),
    )
}

fn locate(ages: &[f32; CURVE_KEYS], age: f32) -> (usize, f32) {
    if age <= ages[0] {
        return (0, 0.0);
    }
    for at in 0..CURVE_KEYS - 1 {
        if age <= ages[at + 1] {
            let span = (ages[at + 1] - ages[at]).max(1e-6);
            return (at, ((age - ages[at]) / span).clamp(0.0, 1.0));
        }
    }
    (CURVE_KEYS - 1, 0.0)
}
