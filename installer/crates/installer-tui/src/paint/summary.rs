use installer_core::{Answers, BASE_PACKAGES, Offer, PACKAGES_FLAKE, PartitionPlan, fcitx5_addons};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{app::App, glyphs::Glyphs, paint::Painter};

const KEY: usize = 18;

pub(super) fn draw(painter: &mut Painter<'_, '_>, area: Rect, app: &App) {
    let mut sheet = Sheet::new(painter);
    let answers = app.answers();

    locale(&mut sheet, answers);
    disk(&mut sheet, answers);
    system(&mut sheet, answers);
    configuration(&mut sheet, answers);
    warnings(&mut sheet, app);

    let lines = sheet.finish();
    painter.render(Paragraph::new(lines), area);
}

fn locale(sheet: &mut Sheet<'_, '_, '_>, answers: &Answers) {
    sheet.heading("Locale and input");
    sheet.row("Locale", answers.locale.name);
    if answers.english_messages && !answers.locale.english {
        sheet.row("Messages", "en_US.UTF-8");
    }
    sheet.row("Keyboard", answers.keyboard.label);
    sheet.row("Time zone", &answers.timezone);
}

fn disk(sheet: &mut Sheet<'_, '_, '_>, answers: &Answers) {
    sheet.heading("Disk");
    let plan = match PartitionPlan::build(answers) {
        Ok(plan) => plan,
        Err(error) => return sheet.message(&error.to_string(), sheet.danger()),
    };

    sheet.row("Target", &plan.disk.describe());
    for entry in plan.describe() {
        sheet.note(&entry);
    }
    sheet.row("Filesystem", answers.filesystem.label());
    sheet.row("Swap", &answers.swap.label());
    sheet.row(
        "Encryption",
        match answers.encrypt {
            true => "LUKS2 (argon2id)",
            false => "none",
        },
    );
}

fn system(sheet: &mut Sheet<'_, '_, '_>, answers: &Answers) {
    sheet.heading("System");
    sheet.row("Hostname", &answers.hostname);
    sheet.row("User", &account(answers));
    sheet.row(
        "Root",
        match answers.set_root_password {
            true => "password set",
            false => "no login; sudo only",
        },
    );
    sheet.row("nixpkgs", &answers.channel.source(&answers.release));
    sheet.row("stateVersion", &answers.release);
    sheet.row("Kernel", answers.kernel.label());
    graphics(sheet, answers);
    sheet.row("OpenSSH", &ssh(answers, &sheet.glyphs()));
    sheet.row("Always on", "NetworkManager, firewall, flakes, weekly GC");
    if answers.sudo_rs {
        sheet.row("sudo", "sudo-rs");
    }
}

fn configuration(sheet: &mut Sheet<'_, '_, '_>, answers: &Answers) {
    sheet.heading("Configuration");
    sheet.row("Yours", "/etc/nixos/{flake,configuration}.nix");
    sheet.row(
        "Rikka",
        "/etc/nixos/rikka/{apps,boot,desktop,ime,nix,packages,system}.nix",
    );
    sheet.row("Distribution", PACKAGES_FLAKE);
    sheet.row("Apps", &apps(answers));
    sheet.row("Packages", &BASE_PACKAGES.join(" "));
    sheet.row("Login", &format!("Rikka on tty1 as {}", answers.username));
    sheet.note("console login when the session exits");
    sheet.row("Input method", &input_method(answers, &sheet.glyphs()));
    sheet.row(
        "Unfree",
        match answers.allow_unfree {
            true => "allowed",
            false => "refused",
        },
    );
}

fn apps(answers: &Answers) -> String {
    let names: Vec<&str> = answers
        .catalog
        .components()
        .iter()
        .filter(|component| {
            component.offer == Offer::Required || answers.apps.contains(&component.id)
        })
        .map(|component| component.name.as_str())
        .collect();
    names.join(", ")
}

fn warnings(sheet: &mut Sheet<'_, '_, '_>, app: &App) {
    let mark = sheet.glyphs().warning;
    for blocker in app.environment().blockers() {
        sheet.blank();
        sheet.message(&format!("{mark} {}", blocker.message), sheet.warn());
    }

    let target = app
        .answers()
        .disk
        .as_ref()
        .map_or("the disk", |disk| disk.path.as_str());
    sheet.blank();
    sheet.message(
        &format!("{mark} Enter starts writing to {target}."),
        sheet.danger().add_modifier(Modifier::BOLD),
    );
}

fn graphics(sheet: &mut Sheet<'_, '_, '_>, answers: &Answers) {
    let mut cards = answers.graphics().into_iter();
    let Some(first) = cards.next() else {
        return;
    };
    sheet.row("Graphics", &first);
    for card in cards {
        sheet.note(&card);
    }
}

fn account(answers: &Answers) -> String {
    if answers.full_name.trim().is_empty() {
        answers.username.clone()
    } else {
        format!("{} ({})", answers.username, answers.full_name.trim())
    }
}

fn ssh(answers: &Answers, glyphs: &Glyphs) -> String {
    if !answers.openssh {
        return "off".to_owned();
    }
    let facts = if answers.ssh_password_auth {
        ["on", "passwords allowed", "no root login"]
    } else {
        ["on", "keys only", "no root login"]
    };
    facts.join(glyphs.separator)
}

fn input_method(answers: &Answers, glyphs: &Glyphs) -> String {
    let addons = fcitx5_addons(answers.locale);
    if addons.is_empty() {
        return "fcitx5 (no engine; the keyboard types this locale)".to_owned();
    }
    format!("fcitx5{}{}", glyphs.separator, addons.join(" "))
}

struct Sheet<'a, 'b, 'c> {
    painter: &'a Painter<'b, 'c>,
    lines: Vec<Line<'static>>,
}

impl<'a, 'b, 'c> Sheet<'a, 'b, 'c> {
    fn new(painter: &'a Painter<'b, 'c>) -> Self {
        Self {
            painter,
            lines: Vec::new(),
        }
    }

    fn glyphs(&self) -> Glyphs {
        self.painter.glyphs()
    }

    fn warn(&self) -> Style {
        Style::new().fg(self.painter.theme().warn)
    }

    fn danger(&self) -> Style {
        Style::new().fg(self.painter.theme().danger)
    }

    fn heading(&mut self, title: &'static str) {
        if !self.lines.is_empty() {
            self.blank();
        }
        self.lines.push(Line::styled(
            format!("  {title}"),
            Style::new()
                .fg(self.painter.theme().accent)
                .add_modifier(Modifier::BOLD),
        ));
    }

    fn row(&mut self, key: &str, value: &str) {
        let theme = self.painter.theme();
        self.lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(self.painter.pad(key, KEY), Style::new().fg(theme.muted)),
            Span::styled(value.to_owned(), Style::new().fg(theme.text)),
        ]));
    }

    fn note(&mut self, text: &str) {
        self.lines.push(Line::styled(
            format!("{}{text}", " ".repeat(KEY + 2)),
            Style::new().fg(self.painter.theme().muted),
        ));
    }

    fn message(&mut self, text: &str, style: Style) {
        self.lines.push(Line::styled(format!("  {text}"), style));
    }

    fn blank(&mut self) {
        self.lines.push(Line::raw(""));
    }

    fn finish(self) -> Vec<Line<'static>> {
        self.lines
    }
}
