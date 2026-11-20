use installer_core::{Problem, Step};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{app::App, paint::Painter};

pub(super) fn draw(painter: &mut Painter<'_, '_>, area: Rect, app: &App, problems: &[Problem]) {
    let theme = painter.theme();
    let glyphs = painter.glyphs();
    let current = app.step();

    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(glyphs.border)
        .border_style(Style::new().fg(theme.border))
        .title(Span::styled(" Steps ", Style::new().fg(theme.muted)));
    let inner = block.inner(area);
    painter.render(block, area);

    let width = inner.width as usize;
    let lines: Vec<Line<'static>> = Step::ALL
        .iter()
        .map(|step| {
            let number = step.index() + 1;
            let behind = step.index() < current.index();
            let unsound = behind && problems.iter().any(|problem| problem.step == *step);

            let (mark, style) = if *step == current {
                (
                    glyphs.current,
                    Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                )
            } else if unsound {
                (glyphs.warning, Style::new().fg(theme.warn))
            } else if behind {
                (glyphs.done, Style::new().fg(theme.ok))
            } else {
                (glyphs.pending, Style::new().fg(theme.muted))
            };

            Line::from(vec![
                Span::raw(" "),
                Span::styled(mark, style),
                Span::styled(
                    format!(
                        " {number} {}",
                        painter.fit(step.title(), width.saturating_sub(5))
                    ),
                    style,
                ),
            ])
        })
        .collect();

    painter.render(Paragraph::new(lines), inner);
}
