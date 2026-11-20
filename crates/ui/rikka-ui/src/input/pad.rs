#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadInput {
    Button {
        button: u32,
        pressed: bool,
    },
    Ring {
        group: usize,
        ring: usize,
        degrees: Option<f32>,
    },
    Strip {
        group: usize,
        strip: usize,
        position: Option<f32>,
    },
    Dial {
        group: usize,
        dial: usize,
        value120: i32,
    },
}
