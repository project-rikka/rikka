mod fields;
mod install;
mod overlay;
mod painter;
mod sidebar;
mod summary;

use installer_core::{Problem, Step};
use installer_sys::Mode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{app::App, overlay::Overlay};

pub(crate) use painter::Painter;

const SIDEBAR: u16 = 20;

const MINIMUM: (u16, u16) = (72, 20);

pub(crate) fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let mut painter = Painter::new(frame, *app.theme(), *app.glyphs());
    let area = painter.area();
    if area.width < MINIMUM.0 || area.height < MINIMUM.1 {
        return too_small(&mut painter, area);
    }

    let [header, middle, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .areas(area);
    let [left, right] =
        Layout::horizontal([Constraint::Length(SIDEBAR), Constraint::Min(1)]).areas(middle);

    let problems = app.all_problems();

    paint_header(&mut painter, header, app);
    sidebar::draw(&mut painter, left, app, &problems);
    body(&mut painter, right, app);
    hints(&mut painter, footer, app, &problems);

    if let Some(overlay) = app.overlay() {
        overlay::draw(&mut painter, area, overlay);
    }
}

fn body(painter: &mut Painter<'_, '_>, area: Rect, app: &mut App) {
    let theme = painter.theme();
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(painter.glyphs().border)
        .border_style(Style::new().fg(theme.border))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                app.step().title(),
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));
    let inner = block.inner(area).inner(ratatui::layout::Margin::new(1, 0));
    painter.render(block, area);

    match app.step() {
        Step::Summary => summary::draw(painter, inner, app),
        Step::Install => install::draw(painter, inner, app),
        _ => fields::draw(painter, inner, app),
    }
}

fn paint_header(painter: &mut Painter<'_, '_>, area: Rect, app: &App) {
    let theme = painter.theme();
    let title = Line::from(vec![
        Span::styled(
            " Rikka ",
            Style::new()
                .fg(theme.on_accent)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Installer", Style::new().fg(theme.muted)),
    ]);
    painter.render(Paragraph::new(title), area);

    let environment = app.environment();
    let mut facts = vec![
        if environment.uefi { "UEFI" } else { "BIOS" }.to_owned(),
        environment.memory.to_string(),
        format!("NixOS {}", environment.release),
        format!(
            "net {}",
            match environment.online {
                true => painter.glyphs().done,
                false => painter.glyphs().failed,
            }
        ),
    ];
    match environment.mode {
        Mode::Live => {}
        Mode::DryRun => facts.push("DRY RUN".to_owned()),
        Mode::Demo => facts.push("DEMO".to_owned()),
    }

    let style = if environment.mode.is_live() {
        Style::new().fg(theme.muted)
    } else {
        Style::new().fg(theme.on_accent).bg(theme.warn)
    };
    painter.render(
        Paragraph::new(Line::styled(
            format!(" {} ", facts.join(painter.glyphs().separator)),
            style,
        ))
        .alignment(Alignment::Right),
        area,
    );
}

fn hints(painter: &mut Painter<'_, '_>, area: Rect, app: &App, problems: &[Problem]) {
    let theme = painter.theme();
    let [message, keys] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);

    let blocking = problems
        .iter()
        .find(|problem| problem.step == app.step())
        .or_else(|| match app.step() {
            Step::Summary | Step::Install => problems.first(),
            _ => None,
        });
    let line = match blocking {
        Some(problem) => Line::styled(
            format!(" {} {}", painter.glyphs().warning, problem.message),
            Style::new().fg(theme.danger),
        ),
        None => Line::raw(""),
    };
    painter.render(Paragraph::new(line), message);

    painter.render(
        Paragraph::new(Line::styled(
            format!("  {}", keymap(app)),
            Style::new().fg(theme.muted),
        )),
        keys,
    );
}

fn keymap(app: &App) -> &'static str {
    match app.overlay() {
        Some(Overlay::Picker(_)) => "type to filter   up/down select   Enter choose   Esc close",
        Some(Overlay::Confirm(_)) => "left/right   y/n   Enter confirm   Esc cancel",
        Some(Overlay::Help) => "any key closes",
        None => match app.step() {
            Step::Install => {
                if app.install().finished() {
                    "up/down log   r reboot   q quit"
                } else {
                    "up/down PgUp/PgDn log   End follow   ^Q abort"
                }
            }
            Step::Summary => "Enter install   Esc back   ^Q quit",
            _ => "Enter next   Tab/arrows move   Space toggle   Esc back   F1 help   ^Q quit",
        },
    }
}

fn too_small(painter: &mut Painter<'_, '_>, area: Rect) {
    let danger = painter.theme().danger;
    painter.render(
        Paragraph::new(format!(
            "This terminal is too small. It needs at least {}x{}.",
            MINIMUM.0, MINIMUM.1
        ))
        .style(Style::new().fg(danger))
        .alignment(Alignment::Center),
        area,
    );
}
