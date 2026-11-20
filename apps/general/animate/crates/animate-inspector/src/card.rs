use animate_body::Side;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    Face(FaceTab),
    Hand(Side),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceTab {
    Eyes,
    Expression,
    Mouth,
}

impl FaceTab {
    pub const ALL: [Self; 3] = [Self::Eyes, Self::Expression, Self::Mouth];
}
