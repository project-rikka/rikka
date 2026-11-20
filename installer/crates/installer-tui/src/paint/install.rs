use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{app::App, paint::Painter, progress::StageState};

pub(super) fn draw(painter: &mut Painter<'_, '_>, area: Rect, app: &mut App) {
    let stages = app.install().stages().len().max(1) as u16;

    let [list, bar, log] = Layout::vertical([
        Constraint::Length(stages + 1),
        Constraint::Length(2),
        Constraint::Min(3),
    ])
    .areas(area);

    stage_list(painter, list, app);
    progress_bar(painter, bar, app);
    log_pane(painter, log, app);
}

fn stage_list(painter: &mut Painter<'_, '_>, area: Rect, app: &App) {
    let theme = painter.theme();
    let glyphs = painter.glyphs();
    let view = app.install();
    let frames = glyphs.spinner;
    let spinner = frames[view.frame() / 2 % frames.len()];

    let lines: Vec<Line<'static>> = view
        .stages()
        .iter()
        .map(|stage| {
            let (mark, style) = match view.state_of(*stage) {
                StageState::Done => (glyphs.done.to_owned(), Style::new().fg(theme.ok)),
                StageState::Running => (
                    spinner.to_owned(),
                    Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                ),
                StageState::Failed => (glyphs.failed.to_owned(), Style::new().fg(theme.danger)),
                StageState::Pending => (glyphs.pending.to_owned(), Style::new().fg(theme.muted)),
            };
            Line::from(vec![
                Span::raw("  "),
                Span::styled(mark, style),
                Span::styled(format!(" {}", stage.title()), style),
            ])
        })
        .collect();

    painter.render(Paragraph::new(lines), area);
}

fn progress_bar(painter: &mut Painter<'_, '_>, area: Rect, app: &App) {
    let theme = painter.theme();
    let glyphs = painter.glyphs();
    let view = app.install();
    let elapsed = view.elapsed().as_secs();
    let clock = format!("{:02}:{:02}", elapsed / 60, elapsed % 60);
    let percent = (view.progress() * 100.0).round() as u32;

    let room = area.width.saturating_sub(4 + 5 + 6 + 2) as usize;
    let full = (view.progress() * room as f32).round() as usize;
    let (colour, label) = match view.outcome() {
        Some(Ok(())) => (theme.ok, "done".to_owned()),
        Some(Err(_)) => (theme.danger, "failed".to_owned()),
        None => (theme.accent, format!("{percent:>3}%")),
    };

    painter.render(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(glyphs.bar_full.repeat(full), Style::new().fg(colour)),
            Span::styled(
                glyphs.bar_empty.repeat(room.saturating_sub(full)),
                Style::new().fg(theme.border),
            ),
            Span::styled(
                format!("  {label}"),
                Style::new().fg(colour).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {clock}"), Style::new().fg(theme.muted)),
        ])),
        area,
    );
}

fn log_pane(painter: &mut Painter<'_, '_>, area: Rect, app: &mut App) {
    let theme = painter.theme();
    let following = app.install().log().following();
    let title = if following {
        " Log ".to_owned()
    } else {
        " Log (scrolled back; End to follow) ".to_owned()
    };

    let block = Block::new()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(theme.border))
        .title(Span::styled(title, Style::new().fg(theme.muted)));
    let inner = block.inner(area);
    painter.render(block, area);

    let height = inner.height as usize;
    app.install_mut().log_mut().set_viewport(height);

    let width = inner.width as usize;
    let lines: Vec<Line<'static>> = app
        .install()
        .log()
        .visible(height)
        .map(|line| {
            let style = if line.starts_with('$') {
                Style::new().fg(theme.accent)
            } else if line.starts_with("(dry-run)") {
                Style::new().fg(theme.warn)
            } else {
                Style::new().fg(theme.muted)
            };
            Line::styled(painter.fit(line, width), style)
        })
        .collect();
    painter.render(Paragraph::new(lines), inner);

    if let Some(outcome) = app.install().outcome() {
        banner(painter, inner, outcome);
    }
}

fn banner(painter: &mut Painter<'_, '_>, area: Rect, outcome: &Result<(), String>) {
    let theme = painter.theme();
    let (text, style) = match outcome {
        Ok(()) => (
            "  Installed. Press r to reboot, q to quit.  ",
            Style::new()
                .fg(theme.on_accent)
                .bg(theme.ok)
                .add_modifier(Modifier::BOLD),
        ),
        Err(_) => (
            "  The install failed. Read the log above. Press q to quit.  ",
            Style::new()
                .fg(theme.on_accent)
                .bg(theme.danger)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let row = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };
    painter.render(Clear, row);
    let padded = painter.pad(text, area.width as usize);
    painter.render(Paragraph::new(Line::styled(padded, style)), row);
}
