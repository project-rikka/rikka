use rikka_seam::Version;
use rikka_types::Rect;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caret {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Caret {
    pub const CAPABILITY: &'static str = "rikka.caret";
    pub const VERSION: Version = Version::new(1, 1);

    pub const WHERE: &'static str = "where";

    pub const MOVED: &'static str = "moved";

    pub const AIM: &'static str = "aim";

    #[must_use]
    pub fn rect(&self) -> Rect {
        Rect::new(
            self.x as f32,
            self.y as f32,
            self.width as f32,
            self.height as f32,
        )
    }
}
