#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Orientation {
    #[default]
    Upright,
    Rotate90,
    Rotate180,
    Rotate270,
    FlipHorizontal,
    FlipVertical,
    Rotate90FlipHorizontal,
    Rotate270FlipHorizontal,
}

type Matrix = [[i8; 2]; 2];

const ALL: [Orientation; 8] = [
    Orientation::Upright,
    Orientation::Rotate90,
    Orientation::Rotate180,
    Orientation::Rotate270,
    Orientation::FlipHorizontal,
    Orientation::FlipVertical,
    Orientation::Rotate90FlipHorizontal,
    Orientation::Rotate270FlipHorizontal,
];

impl Orientation {
    pub(crate) const fn from_exif_value(value: u16) -> Option<Orientation> {
        Some(match value {
            1 => Orientation::Upright,
            2 => Orientation::FlipHorizontal,
            3 => Orientation::Rotate180,
            4 => Orientation::FlipVertical,
            5 => Orientation::Rotate90FlipHorizontal,
            6 => Orientation::Rotate90,
            7 => Orientation::Rotate270FlipHorizontal,
            8 => Orientation::Rotate270,
            _ => return None,
        })
    }

    pub(crate) const fn counter_clockwise(quarters: u8) -> Orientation {
        match quarters % 4 {
            1 => Orientation::Rotate270,
            2 => Orientation::Rotate180,
            3 => Orientation::Rotate90,
            _ => Orientation::Upright,
        }
    }

    pub(crate) fn then(self, next: Orientation) -> Orientation {
        let (first, second) = (self.matrix(), next.matrix());
        let mut product = [[0i8; 2]; 2];
        for (row, out) in product.iter_mut().enumerate() {
            for (column, cell) in out.iter_mut().enumerate() {
                *cell = first[row][0] * second[0][column] + first[row][1] * second[1][column];
            }
        }
        ALL.into_iter()
            .find(|candidate| candidate.matrix() == product)
            .unwrap_or_else(|| unreachable!("orientations compose within their group"))
    }

    const fn matrix(self) -> Matrix {
        match self {
            Orientation::Upright => [[1, 0], [0, 1]],
            Orientation::Rotate90 => [[0, 1], [-1, 0]],
            Orientation::Rotate180 => [[-1, 0], [0, -1]],
            Orientation::Rotate270 => [[0, -1], [1, 0]],
            Orientation::FlipHorizontal => [[-1, 0], [0, 1]],
            Orientation::FlipVertical => [[1, 0], [0, -1]],
            Orientation::Rotate90FlipHorizontal => [[0, 1], [1, 0]],
            Orientation::Rotate270FlipHorizontal => [[0, -1], [-1, 0]],
        }
    }
}
