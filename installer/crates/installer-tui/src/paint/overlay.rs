use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    overlay::{Confirm, Overlay, Picker},
    paint::Painter,
};

pub(super) fn draw(painter: &mut Painter<'_, '_>, screen: Rect, overlay: &Overlay) {
    match overlay {
        Overlay::Picker(picker) => {
            let area = centred(screen, 62, screen.height.saturating_sub(6).min(24));
            picker_box(painter, area, picker);
        }
        Overlay::Confirm(confirm) => {
            let height = u16::try_from(confirm.body.len()).unwrap_or(4) + 6;
            let area = centred(screen, 68, height);
            confirm_box(painter, area, confirm);
        }
        Overlay::Help => help_box(painter, centred(screen, 66, 18)),
    }
}

fn frame_for(painter: &mut Painter<'_, '_>, area: Rect, title: &str, accent: Color) -> Rect {
    let theme = painter.theme();
    painter.render(Clear, area);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(painter.glyphs().border)
        .border_style(Style::new().fg(accent))
        .style(Style::new().bg(theme.on_accent))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(accent).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area).inner(ratatui::layout::Margin::new(1, 0));
    painter.render(block, area);
    inner
}

fn picker_box(painter: &mut Painter<'_, '_>, area: Rect, picker: &Picker) {
    let theme = painter.theme();
    let inner = frame_for(painter, area, &picker.title, theme.accent);
    let [search, list] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(inner);

    painter.render(
        Paragraph::new(Line::from(vec![
            Span::styled("filter: ", Style::new().fg(theme.muted)),
            Span::styled(
                picker.query.clone(),
                Style::new().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled("_", Style::new().fg(theme.accent)),
            Span::styled(
                format!("   {} of {}", picker.len(), picker.total()),
                Style::new().fg(theme.muted),
            ),
        ])),
        search,
    );

    let height = list.height as usize;
    let width = list.width as usize;
    let first = picker.row().saturating_sub(height.saturating_sub(1));

    let lines: Vec<Line<'static>> = picker
        .shown()
        .skip(first)
        .take(height)
        .map(|(index, choice)| {
            let selected = picker.selected() == Some(index);
            let style = if selected {
                Style::new()
                    .fg(theme.on_accent)
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme.text)
            };
            let text = if choice.note.is_empty() {
                choice.label.clone()
            } else {
                format!("{}   {}", choice.label, choice.note)
            };
            Line::styled(painter.pad(&format!(" {text}"), width), style)
        })
        .collect();

    painter.render(Paragraph::new(lines), list);
}

fn confirm_box(painter: &mut Painter<'_, '_>, area: Rect, confirm: &Confirm) {
    let theme = painter.theme();
    let inner = frame_for(painter, area, &confirm.title, theme.danger);
    let [body, buttons] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(inner);

    let lines: Vec<Line<'static>> = confirm
        .body
        .iter()
        .map(|line| Line::styled(line.clone(), Style::new().fg(theme.text)))
        .collect();
    painter.render(Paragraph::new(lines).wrap(Wrap { trim: false }), body);

    let choice = |label: &str, active: bool, colour| {
        Span::styled(
            format!("  {label}  "),
            if active {
                Style::new()
                    .fg(theme.on_accent)
                    .bg(colour)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme.muted)
            },
        )
    };

    painter.render(
        Paragraph::new(Line::from(vec![
            choice("No (n)", !confirm.yes, theme.accent),
            Span::raw("  "),
            choice("Yes (y)", confirm.yes, theme.danger),
        ]))
        .alignment(Alignment::Right),
        buttons,
    );
}

const HELP: &[(&str, &str)] = &[
    ("Enter", "next field, and the next screen from the last one"),
    ("Tab / Shift-Tab", "move between fields"),
    ("Up / Down", "move between fields; rows inside a list"),
    ("Left / Right", "change a value; move the caret"),
    ("Space", "flip a switch, open a list"),
    ("Esc", "back a screen, or close an open list"),
    ("F1 / ?", "this help"),
    ("Ctrl-Q", "quit"),
    ("", ""),
    ("in a list", "type to filter"),
    (
        "while installing",
        "Up/Down PgUp/PgDn scroll the log, End follows it",
    ),
];

const KEY: usize = 18;

fn help_box(painter: &mut Painter<'_, '_>, area: Rect) {
    let theme = painter.theme();
    let inner = frame_for(painter, area, "Keys", theme.accent);
    let width = inner.width as usize;
    let lines: Vec<Line<'static>> = HELP
        .iter()
        .map(|(key, what)| {
            Line::from(vec![
                Span::styled(
                    painter.pad(key, KEY),
                    Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    painter.fit(what, width.saturating_sub(KEY)),
                    Style::new().fg(theme.text),
                ),
            ])
        })
        .collect();
    painter.render(Paragraph::new(lines), inner);
}

fn centred(screen: Rect, width: u16, height: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(height.min(screen.height))])
        .flex(Flex::Center)
        .areas(screen);
    let [area] = Layout::horizontal([Constraint::Length(width.min(screen.width))])
        .flex(Flex::Center)
        .areas(row);
    area
}
