mod field;
mod kernel;

use rikka_simd::Level;

pub const MOST_SHARDS: usize = 255;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FecError {
    #[error("{data} data and {parity} parity shards are more than {MOST_SHARDS}")]
    TooMany { data: usize, parity: usize },
    #[error("a block needs at least one data shard")]
    NoData,
    #[error("only {have} of the {need} shards needed arrived")]
    Short { have: usize, need: usize },
    #[error("shards of one block differ in length")]
    Uneven,
}

#[derive(Clone, Debug)]
pub struct Cauchy {
    data: usize,
    parity: usize,
    rows: Vec<u8>,
    level: Level,
}

impl Cauchy {
    pub fn new(data: usize, parity: usize) -> Result<Cauchy, FecError> {
        if data == 0 {
            return Err(FecError::NoData);
        }
        if data + parity > MOST_SHARDS {
            return Err(FecError::TooMany { data, parity });
        }
        let rows = (0..parity)
            .flat_map(|row| (0..data).map(move |column| coefficient(data, row, column)))
            .collect();
        Ok(Cauchy {
            data,
            parity,
            rows,
            level: Level::detect(),
        })
    }

    #[must_use]
    pub fn data(&self) -> usize {
        self.data
    }

    #[must_use]
    pub fn parity(&self) -> usize {
        self.parity
    }

    pub fn encode(&self, data: &[&[u8]], parity: &mut [Vec<u8>]) -> Result<(), FecError> {
        let length = uniform(data.iter().map(|shard| shard.len()))?;
        if data.len() != self.data {
            return Err(FecError::Short {
                have: data.len(),
                need: self.data,
            });
        }
        for (row, out) in parity.iter_mut().take(self.parity).enumerate() {
            out.clear();
            out.resize(length, 0);
            let coefficients = &self.rows[row * self.data..(row + 1) * self.data];
            kernel::combine(self.level, coefficients, data, out);
        }
        Ok(())
    }

    pub fn reconstruct(&self, shards: &mut [Option<Vec<u8>>]) -> Result<(), FecError> {
        let total = self.data + self.parity;
        let present: Vec<usize> = (0..total.min(shards.len()))
            .filter(|index| shards[*index].is_some())
            .take(self.data)
            .collect();
        if present.len() < self.data {
            return Err(FecError::Short {
                have: present.len(),
                need: self.data,
            });
        }
        let missing: Vec<usize> = (0..self.data)
            .filter(|index| shards[*index].is_none())
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        let length = uniform(
            present
                .iter()
                .filter_map(|index| shards[*index].as_ref().map(Vec::len)),
        )?;
        let inverse = invert(&self.decoding_matrix(&present), self.data);
        let mut restored = Vec::with_capacity(missing.len());
        {
            let sources: Vec<&[u8]> = present
                .iter()
                .filter_map(|index| shards[*index].as_deref())
                .collect();
            for &row in &missing {
                let mut out = vec![0; length];
                let coefficients = &inverse[row * self.data..(row + 1) * self.data];
                kernel::combine(self.level, coefficients, &sources, &mut out);
                restored.push((row, out));
            }
        }
        for (row, out) in restored {
            shards[row] = Some(out);
        }
        Ok(())
    }

    fn decoding_matrix(&self, present: &[usize]) -> Vec<u8> {
        let mut matrix = vec![0; self.data * self.data];
        for (row, &index) in present.iter().enumerate() {
            let target = &mut matrix[row * self.data..(row + 1) * self.data];
            match index < self.data {
                true => target[index] = 1,
                false => {
                    let parity = index - self.data;
                    target
                        .copy_from_slice(&self.rows[parity * self.data..(parity + 1) * self.data]);
                }
            }
        }
        matrix
    }
}

fn coefficient(data: usize, row: usize, column: usize) -> u8 {
    field::inv(((data + row) ^ column) as u8)
}

fn uniform(mut lengths: impl Iterator<Item = usize>) -> Result<usize, FecError> {
    let first = lengths.next().unwrap_or(0);
    match lengths.all(|length| length == first) {
        true => Ok(first),
        false => Err(FecError::Uneven),
    }
}

fn invert(matrix: &[u8], size: usize) -> Vec<u8> {
    let mut work = matrix.to_vec();
    let mut inverse = vec![0; size * size];
    for index in 0..size {
        inverse[index * size + index] = 1;
    }
    for column in 0..size {
        let pivot = (column..size)
            .find(|row| work[row * size + column] != 0)
            .expect("a Cauchy system with distinct shards is invertible");
        if pivot != column {
            for item in 0..size {
                work.swap(pivot * size + item, column * size + item);
                inverse.swap(pivot * size + item, column * size + item);
            }
        }
        let scale = field::inv(work[column * size + column]);
        for item in 0..size {
            work[column * size + item] = field::mul(work[column * size + item], scale);
            inverse[column * size + item] = field::mul(inverse[column * size + item], scale);
        }
        for row in 0..size {
            let factor = work[row * size + column];
            if row == column || factor == 0 {
                continue;
            }
            for item in 0..size {
                work[row * size + item] ^= field::mul(factor, work[column * size + item]);
                inverse[row * size + item] ^= field::mul(factor, inverse[column * size + item]);
            }
        }
    }
    inverse
}
