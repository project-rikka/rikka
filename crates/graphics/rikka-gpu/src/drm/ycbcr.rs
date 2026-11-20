use crate::core::convert;

const STANDARD_DEFINITION_ROWS: u32 = 576;
const SITED_OFFSET: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum YcbcrMatrix {
    Bt601,
    Bt709,
    Bt2020,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum YcbcrRange {
    Full,
    Limited,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChromaSiting {
    Left,
    Centre,
    TopLeft,
    Top,
    BottomLeft,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ycbcr {
    pub matrix: YcbcrMatrix,
    pub range: YcbcrRange,
    pub siting: ChromaSiting,
}

impl Ycbcr {
    pub fn guessed(height: u32) -> Self {
        Self {
            matrix: match height <= STANDARD_DEFINITION_ROWS {
                true => YcbcrMatrix::Bt601,
                false => YcbcrMatrix::Bt709,
            },
            range: YcbcrRange::Limited,
            siting: ChromaSiting::Centre,
        }
    }

    pub(crate) fn rows(&self, bit_depth: u32) -> [[f32; 4]; 3] {
        let (luma, blue) = match self.matrix {
            YcbcrMatrix::Bt601 => (0.299, 0.114),
            YcbcrMatrix::Bt709 => (0.2126, 0.0722),
            YcbcrMatrix::Bt2020 => (0.2627, 0.0593),
        };
        convert::ycbcr_matrix(luma, blue, bit_depth, 1.0, self.range == YcbcrRange::Full)
    }

    pub(crate) fn chroma_offset(&self) -> [f32; 2] {
        let across = match self.siting {
            ChromaSiting::Left | ChromaSiting::TopLeft | ChromaSiting::BottomLeft => SITED_OFFSET,
            ChromaSiting::Centre | ChromaSiting::Top | ChromaSiting::Bottom => 0.0,
        };
        let down = match self.siting {
            ChromaSiting::TopLeft | ChromaSiting::Top => SITED_OFFSET,
            ChromaSiting::BottomLeft | ChromaSiting::Bottom => -SITED_OFFSET,
            ChromaSiting::Left | ChromaSiting::Centre => 0.0,
        };
        [across, down]
    }
}
