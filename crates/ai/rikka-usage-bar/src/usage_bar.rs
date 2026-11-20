use std::time::{SystemTime, UNIX_EPOCH};

use rikka_format::{RelativeTime, relative_time};
use rikka_llm::{Scope, Span, UsageLevel, Window};
use rikka_ui::{Align, Color, Element, Label, Space, Styled, TextStyled, View, palette, text, tr};
use rikka_widgets::{column, progress, row};

#[must_use]
pub struct UsageBar {
    name: Label,
    used: Label,
    share: f32,
    tint: UsageLevel,
    refills: Option<Label>,
    compact: bool,
}

pub fn usage_bar(window: &Window) -> UsageBar {
    UsageBar {
        name: named(window),
        used: used(window),
        share: (window.used_percent() / 100.0) as f32,
        tint: window.usage_level(),
        refills: refills(window),
        compact: false,
    }
}

impl UsageBar {
    pub fn compact(mut self) -> UsageBar {
        self.compact = true;
        self
    }
}

impl View for UsageBar {
    fn build(self) -> Element {
        let UsageBar {
            name,
            used,
            share,
            tint,
            refills,
            compact,
        } = self;
        let name = match compact {
            true => text(name).caption(),
            false => text(name).label().medium(),
        };
        column! {
            row! {
                name.no_wrap().grow(1.0);
                text(used).caption().muted().no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            progress(share).color(tinted(tint));
            refills.map(|refills| text(refills).caption().muted().no_wrap());
        }
        .gap(Space::TIGHT)
        .fill_width()
        .build()
    }
}

impl std::fmt::Debug for UsageBar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UsageBar")
            .field("name", &self.name.as_str())
            .field("share", &self.share)
            .finish_non_exhaustive()
    }
}

fn named(window: &Window) -> Label {
    match window.scope() {
        Scope::Everything => span(window.span()),
        Scope::Part(part) if window.span() == Span::Unnamed => Label::from(part.as_str()),
        Scope::Part(part) => Label::from(format!("{part} · {}", span(window.span()))),
        Scope::Extra => tr!("usage.extra"),
    }
}

fn span(span: Span) -> Label {
    match span {
        Span::Session => tr!("usage.window.session"),
        Span::Daily => tr!("usage.window.daily"),
        Span::Weekly => tr!("usage.window.weekly"),
        Span::Monthly => tr!("usage.window.monthly"),
        Span::Annual => tr!("usage.window.annual"),
        Span::Unnamed => tr!("usage.window.unnamed"),
    }
}

fn used(window: &Window) -> Label {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a share of a hundred, which an i64 always holds"
    )]
    let percent = window.used_percent().floor() as i64;
    tr!("usage.used", percent = percent)
}

fn refills(window: &Window) -> Option<Label> {
    let at = window.resets_at()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    #[expect(
        clippy::cast_possible_wrap,
        reason = "a count of minutes, hours or days until a reset, which an i64 always holds"
    )]
    let left = match relative_time(at.saturating_sub(now)) {
        Some(RelativeTime::Minutes(count)) => tr!("usage.left.minutes", count = count as i64),
        Some(RelativeTime::Hours(count)) => tr!("usage.left.hours", count = count as i64),
        Some(RelativeTime::Days(count)) => tr!("usage.left.days", count = count as i64),
        None => return Some(tr!("usage.resets-now")),
    };
    Some(tr!("usage.resets", left = left.as_str()))
}

fn tinted(level: UsageLevel) -> Color {
    let palette = palette();
    match level {
        UsageLevel::Critical => palette.danger,
        UsageLevel::Warning => palette.warning,
        UsageLevel::Normal => palette.accent,
    }
}
