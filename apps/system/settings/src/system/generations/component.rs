use rikka_app::prelude::*;
use rikka_nix::{Generation, Module};
use rikka_widgets::{column, row};

use crate::notice::notice;
use crate::pending::Pending;
use crate::picker::value_picker;

const LIMIT: &str = "boot.loader.systemd-boot.configurationLimit";

const LIMITS: [&str; 5] = ["5", "10", "20", "50", "100"];

pub(crate) struct Generations {
    generations: Option<Result<Vec<Generation>, String>>,
}

impl Generations {
    pub(crate) fn new() -> Self {
        Self { generations: None }
    }

    pub(crate) fn read(&mut self, listed: Result<Vec<Generation>, String>) {
        self.generations = Some(listed);
    }
}

pub(crate) fn listed() -> Result<Vec<Generation>, String> {
    Generation::all().map_err(|error| error.to_string())
}

pub(crate) fn view(generations: &Generations, pending: &Pending) -> impl View {
    column! {
        boot(pending);
        list(generations, pending);
    }
    .gap(Space::ROOMY)
}

fn boot(pending: &Pending) -> impl View {
    let limit_value = pending
        .get::<i64>(Module::Boot, LIMIT)
        .map(|limit_value| limit_value.to_string());
    section(
        tr!("settings.generations.boot"),
        field(
            tr!("settings.generations.limit"),
            value_picker(
                tr!("settings.generations.every"),
                LIMITS
                    .iter()
                    .map(|limit_value| ((*limit_value).to_owned(), Label::from(*limit_value)))
                    .collect(),
                limit_value.as_deref(),
                |pending: &mut Pending, limit: Option<String>| {
                    write_limit(pending, limit.as_deref())
                },
            ),
        ),
    )
}

fn list(generations: &Generations, pending: &Pending) -> Element {
    let listed = match &generations.generations {
        None => return Element::empty(),
        Some(Ok(listed)) => listed,
        Some(Err(reason)) => {
            return notice(
                MaterialSymbol::CLOUD_OFF,
                palette().warning,
                tr!("settings.generations.unreadable", reason = reason),
            )
            .build();
        }
    };
    if listed.is_empty() {
        return notice(
            MaterialSymbol::HISTORY,
            palette().text_muted,
            tr!("settings.generations.none"),
        )
        .build();
    }

    card(
        column! {
            for (index, generation) in listed.iter().enumerate() {
                if index > 0 {
                    rule();
                }
                entry(generation, pending);
            }
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
    .build()
}

pub(in crate::system) fn write_limit(pending: &mut Pending, limit: Option<&str>) {
    match limit.and_then(|limit_value| limit_value.parse::<i64>().ok()) {
        Some(limit_value) => pending.set(Module::Boot, LIMIT, limit_value),
        None => pending.remove(Module::Boot, LIMIT),
    }
}

fn entry(generation: &Generation, pending: &Pending) -> impl View {
    let built = match generation.build_time {
        seconds if seconds <= 0 => String::new(),
        seconds => rikka_format::format_timestamp(seconds),
    };
    let summary = match (built.is_empty(), generation.version.is_empty()) {
        (false, false) => format!("{built} · {}", generation.version),
        (false, true) => built,
        (true, false) => generation.version.clone(),
        (true, true) => String::new(),
    };
    let number = generation.number;
    row! {
        column! {
            text(tr!(
                "settings.generations.numbered",
                number = &generation.number.to_string()
            ))
            .label()
            .semibold();
            text(summary).caption().muted();
        }
        .gap(Space::HAIR)
        .grow(1.0);
        if generation.current {
            text(tr!("settings.generations.current"))
                .caption()
                .color(TextColor::Success)
                .no_wrap();
        } else {
            button(tr!("settings.generations.rollback"))
                .ghost()
                .enabled(!pending.busy())
                .on_press(move |pending: &mut Pending| pending.rollback(number));
        }
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
}

impl std::fmt::Debug for Generations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Generations")
            .field(
                "generations",
                &self
                    .generations
                    .as_ref()
                    .map(|listed| listed.as_ref().map(Vec::len)),
            )
            .finish_non_exhaustive()
    }
}
