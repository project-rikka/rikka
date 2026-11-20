mod history;
mod meters;

use desk_layout::{Meter, Monitor, Size};
pub(crate) use history::History;
pub(crate) use meters::named;
use rikka_sysmon::Vitals;
use rikka_ui::{
    Align, Element, Label, Length, MaterialSymbol, Space, Styled, Text, TextAlign, TextStyled,
    View, palette, text, tr,
};
use rikka_widgets::{column, column_of, container, row, spacer};

use crate::widgets::card::padded;
use crate::widgets::kit::{Header, LINE, figures, header};
use crate::widgets::live::Live;
use crate::widgets::monitor::history::SAMPLES;
use crate::widgets::monitor::meters::{MeterReading, gauged, uptime};
use crate::widgets::paint::{columns, span, sparkline};
use crate::widgets::quiet::quiet;
use crate::widgets::sources::Sources;

const TASK_MANAGER: &str = "rikka-task-manager";

const SMALL: Widths = Widths {
    name: 34.0,
    value: 44.0,
    gap: Space::TIGHT,
};
const SMALL_BAR: f32 = 5.0;
const MEDIUM: Widths = Widths {
    name: 40.0,
    value: 72.0,
    gap: Space::SNUG,
};
const MEDIUM_SPARK: f32 = 18.0;
const CAPACITY: f32 = 6.0;
const CORES_WIDTH: f32 = 112.0;
const CORE_PITCH: f32 = 5.0;

#[derive(Clone, Copy)]
struct Widths {
    name: f32,
    value: f32,
    gap: Space,
}

struct MeterSample {
    gauged: MeterReading,
    trace: Vec<f32>,
}

impl MeterSample {
    fn level(&self) -> f32 {
        self.gauged
            .level
            .or_else(|| self.trace.last().copied())
            .unwrap_or_default()
    }

    fn graph(&self, gridded: bool) -> Element {
        if self.gauged.meter == Meter::Disk {
            return container(
                span(0.0, self.level(), palette().accent)
                    .height(Length::px(CAPACITY))
                    .fill_width(),
            )
            .align_y(Align::Center)
            .grow(1.0)
            .min_width(0.0)
            .fill_height()
            .build();
        }
        let line = sparkline(self.trace.iter().copied(), SAMPLES, palette().accent);
        match gridded {
            true => line.gridded(),
            false => line,
        }
        .grow(1.0)
        .min_width(0.0)
        .fill_height()
        .build()
    }

    fn line(&self, widths: Widths, mark: impl View, value: Text) -> Element {
        row! {
            text(self.gauged.short())
                .caption()
                .semibold()
                .muted()
                .no_wrap()
                .width(Length::px(widths.name));
            mark;
            value.align(TextAlign::End).no_wrap().width(Length::px(widths.value));
        }
        .gap(widths.gap)
        .align_items(Align::Center)
        .grow(1.0)
        .fill_width()
        .build()
    }
}

#[must_use]
pub(crate) fn view(monitor: &Monitor, size: Size, sources: &Sources) -> Element {
    let drawn = sources.machine.with(|history| {
        let vitals = history.vitals()?;
        let meters: Vec<MeterSample> = monitor
            .meters
            .iter()
            .filter_map(|meter| gauged(*meter, vitals))
            .map(|gauged| MeterSample {
                trace: history.trace(gauged.trace()),
                gauged,
            })
            .collect();
        Some(match size {
            Size::Small => small(vitals, &meters),
            Size::Medium => medium(vitals, &meters),
            Size::Large | Size::ExtraLarge => large(vitals, &meters),
        })
    });
    let drawn = drawn.unwrap_or_else(|| {
        quiet(
            MaterialSymbol::MONITORING,
            tr!("shell.desk.widget.monitor.measuring"),
        )
        .sized(size)
    });
    container(drawn)
        .on_press(|live: &mut Live| live.desk_scene().launch([TASK_MANAGER]))
        .fill()
        .build()
}

fn heading() -> Header {
    header(
        Some(MaterialSymbol::MONITORING),
        tr!("shell.desk.widget.monitor.title"),
    )
}

fn vitals_line(vitals: &Vitals) -> Label {
    let up = uptime(vitals.cpu.uptime).to_string();
    tr!(
        "shell.desk.widget.monitor.summary",
        up = up.as_str(),
        load = format!("{:.2}", vitals.load.one).as_str(),
    )
}

fn listed(media_header: Header, rows: impl IntoIterator<Item = Element>) -> Element {
    padded(
        column! {
            media_header;
            column_of(rows).grow(1.0).fill_width();
        }
        .gap(Space::TIGHT)
        .fill(),
    )
}

fn small(vitals: &Vitals, meters: &[MeterSample]) -> Element {
    let media_header = match vitals.cpu_temperature() {
        Some(celsius) => heading().trailing(format!("{celsius:.0}°")),
        None => heading(),
    };
    let rows = meters.iter().map(|metered| {
        metered.line(
            SMALL,
            span(0.0, metered.level(), palette().accent)
                .height(Length::px(SMALL_BAR))
                .grow(1.0)
                .min_width(0.0),
            figures(metered.gauged.brief()).caption(),
        )
    });
    listed(media_header, rows)
}

fn medium(vitals: &Vitals, meters: &[MeterSample]) -> Element {
    let rows = meters.iter().map(|metered| {
        metered.line(
            MEDIUM,
            container(metered.graph(false))
                .height(Length::px(MEDIUM_SPARK))
                .grow(1.0)
                .min_width(0.0),
            figures(metered.gauged.shown.clone()).label(),
        )
    });
    listed(heading().trailing(vitals_line(vitals)), rows)
}

fn large(vitals: &Vitals, meters: &[MeterSample]) -> Element {
    let blocks = meters.iter().map(|metered| {
        let gauged = &metered.gauged;
        let cores = (gauged.meter == Meter::Cpu && vitals.cpu.cores.len() > 1).then(|| {
            let width = (vitals.cpu.cores.len() as f32 * CORE_PITCH).min(CORES_WIDTH);
            columns(vitals.cpu.cores.iter().copied(), palette().accent)
                .width(Length::px(width))
                .fill_height()
        });
        column! {
            row! {
                text(gauged.label()).label().semibold().no_wrap();
                if let Some(detail) = &gauged.detail {
                    text(detail.clone()).caption().muted().no_wrap().shrink(1.0);
                }
                spacer().grow(1.0).min_width(Space::SNUG.px());
                figures(gauged.shown.clone())
                    .label()
                    .semibold()
                    .no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            row! {
                metered.graph(true);
                cores;
            }
            .gap(Space::SNUG)
            .grow(1.0)
            .fill_width();
        }
        .gap(LINE)
        .grow(1.0)
        .fill_width()
        .build()
    });
    padded(
        column! {
            heading().trailing(vitals_line(vitals));
            column_of(blocks).gap(Space::SNUG).grow(1.0).fill_width();
        }
        .gap(Space::SNUG)
        .fill(),
    )
}
