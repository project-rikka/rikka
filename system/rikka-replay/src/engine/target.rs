use rikka_capture::{ScreenOutput, ScreenWindow, Source};
use rikka_session::replay::Kind;
use rikka_settings::session::replay::Subject;

use crate::options::CaptureOptions;

pub(crate) const MAX_SUBJECTS: usize = 8;

const DISPLAY: &str = "display:";
const WINDOW: &str = "window:";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WindowInfo {
    pub(crate) id: String,
    pub(crate) output: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub(crate) subject: String,
    pub(crate) source: Source,
    pub(crate) kind: Kind,
    pub(crate) canvas: Option<(u32, u32)>,
}

#[must_use]
pub(crate) fn display_subject(name: &str) -> String {
    format!("{DISPLAY}{name}")
}

#[must_use]
pub(crate) fn window_subject(identifier: &str) -> String {
    format!("{WINDOW}{identifier}")
}

#[must_use]
pub(crate) fn targets(
    capture: &CaptureOptions,
    outputs: &[ScreenOutput],
    windows: &[ScreenWindow],
    window_info: &[WindowInfo],
) -> Vec<Target> {
    match capture.subject {
        Subject::Screen => outputs
            .iter()
            .take(MAX_SUBJECTS)
            .map(|output| Target {
                subject: display_subject(&output.name),
                source: Source::Output(output.name.clone()),
                kind: Kind::Display {
                    name: output.name.clone(),
                    description: output.description.clone(),
                },
                canvas: None,
            })
            .collect(),
        Subject::Apps | Subject::Windows => {
            let select_entry = |window: &&ScreenWindow| {
                capture.subject == Subject::Windows
                    || capture.apps.iter().any(|app| app == &window.app_id)
            };
            let rank = |window: &ScreenWindow| {
                window_info
                    .iter()
                    .position(|info| info.id == window.identifier)
                    .unwrap_or(usize::MAX)
            };
            let mut matching: Vec<&ScreenWindow> = windows.iter().filter(select_entry).collect();
            matching.sort_by_key(|window| rank(window));
            matching
                .into_iter()
                .take(MAX_SUBJECTS)
                .map(|window| {
                    let output = window_info
                        .iter()
                        .find(|info| info.id == window.identifier)
                        .map(|info| info.output.as_str());
                    Target {
                        subject: window_subject(&window.identifier),
                        source: Source::Window(window.identifier.clone()),
                        kind: Kind::Window {
                            app_id: window.app_id.clone(),
                            title: window.title.clone(),
                        },
                        canvas: Some(canvas_size(output, outputs)),
                    }
                })
                .collect()
        }
    }
}

fn canvas_size(output: Option<&str>, outputs: &[ScreenOutput]) -> (u32, u32) {
    let on = output.and_then(|name| outputs.iter().find(|output| output.name == name));
    let largest = || {
        outputs
            .iter()
            .max_by_key(|output| u64::from(output.width) * u64::from(output.height))
    };
    on.or_else(largest).map_or((1920, 1080), |output| {
        (output.width & !1, output.height & !1)
    })
}
