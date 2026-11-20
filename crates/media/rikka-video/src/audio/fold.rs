const SIDE: f32 = std::f32::consts::FRAC_1_SQRT_2;

#[derive(Clone, Debug)]
pub(crate) struct Fold {
    from: usize,
    weights: Vec<f32>,
}

impl Fold {
    pub(crate) fn new(from: usize, to: usize) -> Fold {
        let weights = match (from, to) {
            (from, to) if from == to => Vec::new(),
            (1, to) => vec![1.0; to],
            (_, 1) => vec![1.0 / from as f32; from],
            (from, _) => stereo(from),
        };
        Fold { from, weights }
    }

    pub(crate) fn input_channels(&self) -> usize {
        self.from
    }

    pub(crate) fn apply(&self, input: &[f32], output: &mut Vec<f32>) {
        if self.weights.is_empty() {
            output.extend_from_slice(input);
            return;
        }
        for frame in input.chunks_exact(self.from) {
            for row in self.weights.chunks_exact(self.from) {
                output.push(
                    frame
                        .iter()
                        .zip(row)
                        .map(|(sample, weight)| sample * weight)
                        .sum(),
                );
            }
        }
    }
}

fn stereo(from: usize) -> Vec<f32> {
    let roles = roles(from);
    let mut left = vec![0.0; from];
    let mut right = vec![0.0; from];
    for (channel, role) in roles.iter().enumerate() {
        let (to_left, to_right) = match role {
            Role::Left => (1.0, 0.0),
            Role::Right => (0.0, 1.0),
            Role::Centre => (SIDE, SIDE),
            Role::LeftBehind => (SIDE, 0.0),
            Role::RightBehind => (0.0, SIDE),
            Role::Low => (0.0, 0.0),
        };
        left[channel] = to_left;
        right[channel] = to_right;
    }
    let loudest = left.iter().sum::<f32>().max(right.iter().sum::<f32>());
    left.iter_mut()
        .chain(right.iter_mut())
        .for_each(|weight| *weight /= loudest);
    left.into_iter().chain(right).collect()
}

#[derive(Clone, Copy, Debug)]
enum Role {
    Left,
    Right,
    Centre,
    Low,
    LeftBehind,
    RightBehind,
}

fn roles(channels: usize) -> Vec<Role> {
    use Role::*;
    let known: &[Role] = match channels {
        3 => &[Left, Right, Centre],
        4 => &[Left, Right, LeftBehind, RightBehind],
        5 => &[Left, Right, Centre, LeftBehind, RightBehind],
        _ => &[
            Left,
            Right,
            Centre,
            Low,
            LeftBehind,
            RightBehind,
            LeftBehind,
            RightBehind,
        ],
    };
    (0..channels)
        .map(|channel| match known.get(channel) {
            Some(role) => *role,
            None if channel % 2 == 0 => LeftBehind,
            None => RightBehind,
        })
        .collect()
}
