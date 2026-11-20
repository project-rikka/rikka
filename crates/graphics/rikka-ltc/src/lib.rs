//! Linearly transformed cosine tables for GGX area lights (Heitz et al. 2016, 2017).
//!
//! [`fit`] fits the tables deterministically; [`GGX`] holds the committed result that the
//! renderer uploads, regenerated with `bazel run //xtask -- ltc`.

mod fit;
mod ggx;
mod lobe;
mod simplex;

/// Entries per axis of the square tables.
pub const SIZE: usize = 64;

/// Bytes per entry in [`Table::to_bytes`]: eight little-endian `f32`s.
pub const ENTRY_BYTES: usize = 32;

/// The committed GGX table, `SIZE × SIZE` entries in [`Table::to_bytes`] layout.
pub static GGX: &[u8] = include_bytes!("../data/ggx.bin");

/// One texel of the tables.
///
/// The matrix and amplitudes are indexed by perceptual roughness (x, `a / (SIZE - 1)`) and
/// `sqrt(1 - n·v)` (y); `sphere` is indexed by the cosine between a sphere's direction and the
/// normal (x, `2 i / (SIZE - 1) - 1`) and the sphere's unclipped form factor (y).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Entry {
    /// Columns 0 and 2 of the inverse transform, `(m00, m02, m20, m22)` with `m11` scaled to 1.
    pub inverse: [f32; 4],
    /// Directional albedo of the GGX lobe with a white Fresnel term.
    pub magnitude: f32,
    /// Schlick weight of the directional albedo, so `f0 · magnitude + (1 - f0) · fresnel`.
    pub fresnel: f32,
    /// Horizon-clipped form factor of a sphere divided by its unclipped form factor.
    pub sphere: f32,
}

/// The fitted tables in row-major order: index `y * SIZE + x`.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub entries: Vec<Entry>,
}

impl Table {
    /// Parses [`Table::to_bytes`] output; `None` when the length is not `SIZE² × ENTRY_BYTES`.
    pub fn from_bytes(bytes: &[u8]) -> Option<Table> {
        if bytes.len() != SIZE * SIZE * ENTRY_BYTES {
            return None;
        }
        let entries = bytes
            .as_chunks::<ENTRY_BYTES>()
            .0
            .iter()
            .map(|entry| {
                let value = |index: usize| {
                    let at = index * 4;
                    f32::from_le_bytes([entry[at], entry[at + 1], entry[at + 2], entry[at + 3]])
                };
                Entry {
                    inverse: [value(0), value(1), value(2), value(3)],
                    magnitude: value(4),
                    fresnel: value(5),
                    sphere: value(6),
                }
            })
            .collect();
        Some(Table { entries })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.entries
            .iter()
            .flat_map(|entry| {
                let [a, b, c, d] = entry.inverse;
                [
                    a,
                    b,
                    c,
                    d,
                    entry.magnitude,
                    entry.fresnel,
                    entry.sphere,
                    0.0,
                ]
            })
            .flat_map(f32::to_le_bytes)
            .collect()
    }

    pub fn entry(&self, x: usize, y: usize) -> Entry {
        self.entries[y * SIZE + x]
    }
}

/// Fits the GGX matrices of every roughness from `lowest` (an index below `SIZE`) up to 1.
///
/// Returns rows indexed by roughness: element `k` holds roughness `lowest + k` over every
/// view angle. Each roughness starts from the fit one step rougher, so a row depends only on
/// the rows above it and is reproducible on its own.
pub fn fit_roughness(lowest: usize) -> Vec<Vec<Entry>> {
    let mut axes = Vec::with_capacity(SIZE - lowest);
    let mut previous = None;
    for roughness in (lowest..SIZE).rev() {
        let (lobe, entry) = fit::axis(roughness, previous.as_ref());
        axes.push((roughness, lobe, entry));
        previous = Some(lobe);
    }
    axes.reverse();
    let workers = std::thread::available_parallelism().map_or(1, |count| count.get());
    let mut rows = vec![Vec::new(); axes.len()];
    std::thread::scope(|scope| {
        for (chunk, slots) in axes
            .chunks(axes.len().div_ceil(workers))
            .zip(rows.chunks_mut(axes.len().div_ceil(workers)))
        {
            scope.spawn(move || {
                for (&(roughness, lobe, entry), slot) in chunk.iter().zip(slots) {
                    *slot = fit::row(roughness, lobe, entry);
                }
            });
        }
    });
    rows
}

/// Horizon-clipped sphere form factor ratio for one cosine and unclipped form factor.
pub fn sphere(cosine: f64, form_factor: f64) -> f32 {
    fit::sphere(cosine, form_factor) as f32
}

/// Fits the whole table, matrices and sphere clipping alike.
pub fn fit() -> Table {
    let rows = fit_roughness(0);
    let mut entries = vec![Entry::default(); SIZE * SIZE];
    for (x, row) in rows.iter().enumerate() {
        for (y, entry) in row.iter().enumerate() {
            entries[y * SIZE + x] = Entry {
                sphere: sphere(
                    2.0 * x as f64 / (SIZE - 1) as f64 - 1.0,
                    y as f64 / (SIZE - 1) as f64,
                ),
                ..*entry
            };
        }
    }
    Table { entries }
}
