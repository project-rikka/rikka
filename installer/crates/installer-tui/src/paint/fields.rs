use installer_core::Disk;
use ratatui::{
    layout::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    app::App,
    form::{Field, TextValue, Tone},
    glyphs::Glyphs,
    paint::Painter,
};

const LABEL: usize = 18;

pub(super) fn draw(painter: &mut Painter<'_, '_>, area: Rect, app: &mut App) {
    let theme = painter.theme();
    let width = area.width as usize;
    let focus = app.form().focus();

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut focus_row = 0;
    let mut caret = None;

    for (index, field) in app.form().fields().iter().enumerate() {
        let focused = index == focus;
        if focused {
            focus_row = lines.len();
            caret = caret_of(field, lines.len(), &painter.glyphs());
        }
        paint(&mut lines, field, focused, painter, width);
        lines.push(Line::raw(""));
    }

    for (text, tone) in app.detail() {
        lines.push(Line::styled(
            format!("  {}", painter.fit(&text, width.saturating_sub(2))),
            Style::new().fg(match tone {
                Tone::Plain => theme.muted,
                Tone::Warning => theme.warn,
                Tone::Danger => theme.danger,
            }),
        ));
    }

    let height = area.height as usize;
    let scroll = focus_row
        .saturating_sub(height.saturating_sub(3))
        .min(lines.len().saturating_sub(height));

    painter.render(
        Paragraph::new(lines).scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0)),
        area,
    );

    if let Some((row, column)) = caret
        && row >= scroll
        && row - scroll < height
    {
        painter.caret(Position::new(
            area.x + u16::try_from(column).unwrap_or(0),
            area.y + u16::try_from(row - scroll).unwrap_or(0),
        ));
    }
}

fn caret_of(field: &Field, row: usize, glyphs: &Glyphs) -> Option<(usize, usize)> {
    let Field::Text { value, cursor, .. } = field else {
        return None;
    };
    let columns = match value {
        TextValue::Hidden(_) => cursor * glyphs.mask.width(),
        TextValue::Plain(text) => text
            .chars()
            .take(*cursor)
            .filter_map(UnicodeWidthChar::width)
            .sum(),
    };
    Some((row, LABEL + 2 + columns))
}

fn paint(
    lines: &mut Vec<Line<'static>>,
    field: &Field,
    focused: bool,
    painter: &Painter<'_, '_>,
    width: usize,
) {
    let theme = painter.theme();
    let label_style = if focused {
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.text)
    };
    let value_style = if focused {
        Style::new().fg(theme.text).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.text)
    };
    let box_style = Style::new().fg(if focused { theme.accent } else { theme.border });
    let room = width.saturating_sub(LABEL + 4);

    match field {
        Field::Text {
            label,
            value,
            placeholder,
            ..
        } => {
            let shown = match value {
                TextValue::Hidden(secret) => painter.glyphs().mask.repeat(secret.chars()),
                TextValue::Plain(text) => text.clone(),
            };
            let (text, style) = if shown.is_empty() {
                ((*placeholder).to_owned(), Style::new().fg(theme.muted))
            } else {
                (shown, value_style)
            };
            lines.push(Line::from(vec![
                Span::styled(painter.pad(label, LABEL), label_style),
                Span::styled("[ ", box_style),
                Span::styled(painter.pad(&text, room), style),
                Span::styled(" ]", box_style),
            ]));
        }

        Field::Choice {
            label,
            options,
            index,
            ..
        }
        | Field::Picker {
            label,
            options,
            index,
            ..
        } => {
            let chosen = options.get(*index);
            let text = chosen
                .map(|choice| choice.label.clone())
                .unwrap_or_default();
            let note = chosen.map(|choice| choice.note.clone()).unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled(painter.pad(label, LABEL), label_style),
                Span::styled("[ ", box_style),
                Span::styled(painter.pad(&text, room.saturating_sub(2)), value_style),
                Span::styled(format!("{} ]", painter.glyphs().dropdown), box_style),
                Span::styled(
                    if note.is_empty() {
                        String::new()
                    } else {
                        format!("  {note}")
                    },
                    Style::new().fg(theme.muted),
                ),
            ]));
        }

        Field::Toggle { label, on, .. } => {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    painter.glyphs().toggle(*on),
                    Style::new().fg(if *on { theme.accent } else { theme.border }),
                ),
                Span::raw(" "),
                Span::styled(painter.fit(label, width.saturating_sub(6)), label_style),
            ]));
        }

        Field::Disks {
            label,
            disks,
            index,
            ..
        } => {
            lines.push(Line::styled(painter.pad(label, LABEL), label_style));
            if disks.is_empty() {
                lines.push(Line::styled(
                    "  No writable disk found.",
                    Style::new().fg(theme.danger),
                ));
            }
            for (row, disk) in disks.iter().enumerate() {
                lines.push(disk_row(disk, row == *index, focused, painter, width));
            }
        }
    }
}

fn disk_row(
    disk: &Disk,
    selected: bool,
    focused: bool,
    painter: &Painter<'_, '_>,
    width: usize,
) -> Line<'static> {
    let theme = painter.theme();
    let glyphs = painter.glyphs();
    let mark = if selected {
        glyphs.selected
    } else {
        glyphs.unselected
    };
    let style = match (selected, focused) {
        (true, true) => Style::new()
            .fg(theme.on_accent)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD),
        (true, false) => Style::new().fg(theme.accent),
        _ => Style::new().fg(theme.text),
    };

    let mut tail = disk.kind.label().to_owned();
    if disk.in_use {
        tail.push_str(glyphs.separator);
        tail.push_str("mounted");
    }
    if disk.read_only {
        tail.push_str(glyphs.separator);
        tail.push_str("read-only");
    }

    let body = format!(
        "{mark} {:<12} {:>9}  {}",
        disk.name,
        disk.size.to_string(),
        painter.fit(&disk.model, 28)
    );
    let room = width.saturating_sub(2);
    let padded = painter.pad(&body, room.saturating_sub(tail.width() + 2));

    Line::from(vec![
        Span::raw("  "),
        Span::styled(padded, style),
        Span::styled(format!("  {tail}"), Style::new().fg(theme.muted)),
    ])
}
