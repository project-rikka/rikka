use std::sync::Arc;

use rikka_color_formats::Rgb;
use rikka_ui::ImageSource;
use shell_capture::Panel;

#[derive(Clone)]
pub(crate) struct Frame {
    pub(crate) output: String,
    pub(crate) picture: ImageSource,
    pixels: Arc<[u8]>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Frame {
    #[must_use]
    pub(crate) fn from_panel(panel: Panel) -> Frame {
        let pixels: Arc<[u8]> = Arc::from(panel.pixels);
        Frame {
            output: panel.name,
            picture: ImageSource::Rgba {
                width: panel.width,
                height: panel.height,
                pixels: Arc::clone(&pixels),
            },
            pixels,
            width: panel.width,
            height: panel.height,
        }
    }

    #[must_use]
    pub(crate) fn at(&self, x: u32, y: u32) -> Rgb {
        let start = (y as usize * self.width as usize + x as usize) * 4;
        match self.pixels.get(start..start + 3) {
            Some(&[r, g, b]) => Rgb::new(r, g, b),
            _ => Rgb::default(),
        }
    }
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("output", &self.output)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}
