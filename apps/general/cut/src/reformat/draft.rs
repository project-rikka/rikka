use cut_project::{Format, Rate};

pub(crate) const SIZES: [(u32, u32); 7] = [
    (3840, 2160),
    (2560, 1440),
    (1920, 1080),
    (1280, 720),
    (1080, 1920),
    (1080, 1350),
    (1080, 1080),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Reformatting {
    pub(crate) format: Format,
    pub(crate) original: Format,
}

impl Reformatting {
    pub(crate) fn new(original: Format) -> Reformatting {
        Reformatting {
            format: original,
            original,
        }
    }

    pub(crate) fn size(&self) -> Option<usize> {
        SIZES
            .iter()
            .position(|size| *size == (self.format.width, self.format.height))
    }

    pub(crate) fn rates(&self) -> Vec<Rate> {
        let mut rates = Rate::COMMON.to_vec();
        if !rates.contains(&self.original.rate) {
            rates.push(self.original.rate);
        }
        rates
    }

    pub(crate) fn sized(&mut self, width: u32, height: u32) {
        let side = |value: u32| {
            let even = value.div_ceil(2) * 2;
            even.clamp(*Format::SIDES.start(), *Format::SIDES.end())
        };
        self.format.width = side(width);
        self.format.height = side(height);
    }

    pub(crate) fn turned(&mut self) {
        self.sized(self.format.height, self.format.width);
    }
}
