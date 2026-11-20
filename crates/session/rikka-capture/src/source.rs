#[derive(Clone, Debug)]
pub struct ScreenOutput {
    pub name: String,
    pub description: String,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub x: i32,
    pub y: i32,
    pub scale: i32,
    pub logical_width: u32,
    pub logical_height: u32,
}

impl ScreenOutput {
    pub(crate) fn new() -> ScreenOutput {
        ScreenOutput {
            name: String::new(),
            description: String::new(),
            width: 0,
            height: 0,
            refresh: 0,
            x: 0,
            y: 0,
            scale: 1,
            logical_width: 0,
            logical_height: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScreenWindow {
    pub identifier: String,
    pub title: String,
    pub app_id: String,
}

#[derive(Clone, Debug)]
pub enum Source {
    Output(String),
    FirstOutput,
    Window(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn fits(&self, width: u32, height: u32) -> bool {
        self.x >= 0
            && self.y >= 0
            && self.width > 0
            && self.height > 0
            && self.x as u32 + self.width <= width
            && self.y as u32 + self.height <= height
    }
}
