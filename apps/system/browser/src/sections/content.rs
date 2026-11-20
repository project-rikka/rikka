use rikka_app::prelude::*;
use rikka_syntax::Language;
use rikka_web::{Autoplay, ColorScheme, Spellcheck};
use rikka_widgets::{column, row};

use crate::sections::engine::{change, choice_field, current, title};
use crate::sections::languages;
use crate::shell::Shell;

const STYLE_LINES: std::ops::RangeInclusive<f32> = 120.0..=360.0;

fn autoplay_name(autoplay: Autoplay) -> Label {
    match autoplay {
        Autoplay::BlockAudible => tr!("browser.content.autoplay-block-audible"),
        Autoplay::BlockAll => tr!("browser.content.autoplay-block-all"),
        Autoplay::Allowed => tr!("browser.content.autoplay-allowed"),
    }
}

fn scheme_name(scheme: ColorScheme) -> Label {
    match scheme {
        ColorScheme::System => tr!("browser.content.scheme-system"),
        ColorScheme::Light => tr!("browser.content.scheme-light"),
        ColorScheme::Dark => tr!("browser.content.scheme-dark"),
    }
}

fn spellcheck_name(spellcheck: Spellcheck) -> Label {
    match spellcheck {
        Spellcheck::MultiLine => tr!("browser.content.spellcheck-multi-line"),
        Spellcheck::Everywhere => tr!("browser.content.spellcheck-everywhere"),
        Spellcheck::Off => tr!("browser.content.spellcheck-off"),
    }
}

impl Shell {
    fn apply_user_style(&mut self) {
        let style = self.sections.web.style.text();
        change(self, |settings| settings.user_style = style);
    }

    fn clear_user_style(&mut self) {
        self.sections.web.style.reset("");
        change(self, |settings| settings.user_style.clear());
    }
}

fn user_style(shell: &Shell) -> Element {
    let saved = current(shell).user_style;
    let document = &shell.sections.web.style;
    column! {
        text_editor(document)
            .placeholder(tr!("browser.content.style-placeholder"))
            .code(true)
            .language(Language::named("css"))
            .fits_text(STYLE_LINES)
            .on_change(|| {})
            .fill_width();
        row! {
            button(text(tr!("browser.content.style-apply")))
                .secondary()
                .small()
                .on_press(|shell: &mut Shell| shell.apply_user_style());
            button(text(tr!("browser.content.style-clear")))
                .ghost()
                .small()
                .enabled(!saved.is_empty())
                .on_press(|shell: &mut Shell| shell.clear_user_style());
        }
        .gap(Space::SNUG);
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

pub(crate) fn view(shell: &Shell) -> Element {
    let settings = current(shell);
    let interface = rikka_i18n::current_locale().to_string();
    page([
        section(
            title(shell, tr!("browser.content.languages")),
            languages::list(
                &settings.page_languages,
                tr!("browser.content.languages-interface", language = &interface),
                |settings| &mut settings.page_languages,
            ),
        )
        .any(),
        section(
            title(shell, tr!("browser.content.media")),
            column! {
                choice_field(
                    tr!("browser.content.autoplay"),
                    settings.autoplay,
                    autoplay_name,
                    |settings, autoplay| settings.autoplay = autoplay,
                );
                choice_field(
                    tr!("browser.content.scheme"),
                    settings.color_scheme,
                    scheme_name,
                    |settings, scheme| settings.color_scheme = scheme,
                );
            }
            .gap(Space::NORMAL)
            .fill_width(),
        )
        .any(),
        section(
            title(shell, tr!("browser.content.spelling")),
            column! {
                choice_field(
                    tr!("browser.content.spellcheck"),
                    settings.spellcheck,
                    spellcheck_name,
                    |settings, spellcheck| settings.spellcheck = spellcheck,
                );
                text(tr!("browser.content.dictionaries")).label();
                languages::list(
                    &settings.dictionaries,
                    tr!("browser.content.dictionaries-automatic"),
                    |settings| &mut settings.dictionaries,
                );
            }
            .gap(Space::NORMAL)
            .fill_width(),
        )
        .any(),
        section(
            title(shell, tr!("browser.content.style")),
            user_style(shell),
        )
        .any(),
    ])
    .any()
}
