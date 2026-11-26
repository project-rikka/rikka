use browser_preferences::{ReaderStyle, ReaderTheme};
use browser_reader::{Article, Labels, Look, Palette};
use rikka_app::prelude::*;

pub(crate) fn look(style: ReaderStyle, dark: bool) -> Look {
    let palette = match style.theme {
        ReaderTheme::Auto if dark => Palette::Dark,
        ReaderTheme::Auto | ReaderTheme::Light => Palette::Light,
        ReaderTheme::Sepia => Palette::Sepia,
        ReaderTheme::Dark => Palette::Dark,
    };
    Look {
        palette,
        typeface: style.typeface,
        size: style.size,
        width: style.width,
        spacing: style.spacing,
    }
}

pub(crate) fn labels(article: &Article) -> Labels {
    let minutes = article.reading().minutes.to_string();
    Labels {
        reading_time: Some(tr!("browser.reader.minutes", minutes = &minutes).to_string()),
        published: article.published().map(str::to_owned),
    }
}
