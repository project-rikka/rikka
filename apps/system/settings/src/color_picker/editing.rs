use rikka_app::prelude::*;
use rikka_color_formats::{NEW_FORMAT, Rgb, Template};
use rikka_settings::session::color_picker::{Format, FormatError, validate_formats};

#[derive(Debug)]
pub(crate) struct Editing {
    pub(crate) at: Option<usize>,
    pub(crate) name: TextDocument,
    pub(crate) format: TextDocument,
    pub(crate) error: Option<FormatError>,
    pub(crate) explained: bool,
}

impl Editing {
    pub(crate) fn adding() -> Editing {
        Editing {
            at: None,
            name: TextDocument::new(""),
            format: TextDocument::new(NEW_FORMAT),
            error: None,
            explained: false,
        }
    }

    pub(crate) fn changing(at: usize, format: &Format) -> Editing {
        Editing {
            at: Some(at),
            name: TextDocument::new(format.name.as_str()),
            format: TextDocument::new(format.format.as_str()),
            error: None,
            explained: false,
        }
    }

    pub(crate) fn example(&self) -> String {
        Template::parse(&self.format.text()).render(Rgb::MOCCASIN)
    }

    pub(crate) fn place(&self, formats: &[Format]) -> usize {
        self.at
            .filter(|at| *at < formats.len())
            .unwrap_or(formats.len())
    }

    pub(crate) fn settled(&mut self, formats: &[Format]) -> Option<Vec<Format>> {
        let place = self.place(formats);
        let shown = formats.get(place).is_none_or(|format| format.shown);
        let format = Format {
            shown,
            ..Format::new(self.name.text().trim(), self.format.text())
        };
        let mut reordered = formats.to_vec();
        match reordered.get_mut(place) {
            Some(replaced) => *replaced = format,
            None => reordered.push(format),
        }
        self.error = validate_formats(&reordered).err();
        self.error.is_none().then_some(reordered)
    }
}
