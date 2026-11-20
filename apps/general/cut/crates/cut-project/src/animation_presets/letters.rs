#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Order {
    Forward,
    Backward,
    Centre,
    Random(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Letters {
    pub delay: f32,
    pub order: Order,
}

impl Letters {
    pub const DELAY: f32 = 2.0;

    #[must_use]
    pub fn new(order: Order) -> Letters {
        Letters {
            delay: Letters::DELAY,
            order,
        }
    }

    pub(crate) fn checked(self) -> Letters {
        Letters {
            delay: match self.delay.is_finite() {
                true => self.delay.max(0.0),
                false => Letters::DELAY,
            },
            order: self.order,
        }
    }

    pub(crate) fn ranks(&self, count: usize) -> Vec<usize> {
        match self.order {
            Order::Forward => (0..count).collect(),
            Order::Backward => (0..count).rev().collect(),
            Order::Centre => {
                let middle = (count as f64 - 1.0) / 2.0;
                (0..count)
                    .map(|index| (index as f64 - middle).abs().floor() as usize)
                    .collect()
            }
            Order::Random(seed) => {
                let mut order: Vec<usize> = (0..count).collect();
                order.sort_by_key(|index| mixed(u64::from(seed) << 32 ^ *index as u64));
                let mut ranks = vec![0; count];
                for (rank, index) in order.into_iter().enumerate() {
                    ranks[index] = rank;
                }
                ranks
            }
        }
    }
}

pub(crate) fn mixed(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[must_use]
pub fn letter_count(text: &str) -> usize {
    text.chars()
        .filter(|letter| !letter.is_whitespace())
        .count()
}
