use crate::codec::{counted, record, tagged, within};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WeightedNormalMode {
    #[default]
    FaceArea,
    CornerAngle,
    FaceAreaAngle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeightedNormal {
    pub mode: WeightedNormalMode,
    pub weight: u8,
    pub threshold: f32,
    pub keep_sharp: bool,
    pub use_face_influence: bool,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
}

impl Default for WeightedNormal {
    fn default() -> WeightedNormal {
        WeightedNormal {
            mode: WeightedNormalMode::FaceArea,
            weight: 50,
            threshold: 0.01,
            keep_sharp: false,
            use_face_influence: false,
            vertex_group: String::new(),
            invert_vertex_group: false,
        }
    }
}

tagged!(WeightedNormalMode, "weighted normal mode" {
    FaceArea = 0,
    CornerAngle = 1,
    FaceAreaAngle = 2,
});

record!(WeightedNormal {
    mode,
    weight => counted(1, 100, "weighted normal weight is outside 1..100"),
    threshold => within(0.0, 10.0, "weighted normal threshold is outside 0..10"),
    keep_sharp,
    use_face_influence,
    vertex_group,
    invert_vertex_group,
});
