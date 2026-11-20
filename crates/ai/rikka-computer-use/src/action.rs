use std::time::Duration;

use rikka_computer::{Display, Region, ScrollDirection, Shell};
use rikka_input::Chord;
use rikka_prompt::Prompt;
use rikka_tool::Value;
use rikka_tools::Result;
use rikka_types::{Modifiers, PointerButton};
use serde::Deserialize;

type Point = (u32, u32);

#[derive(Clone, Copy, Debug)]
pub(crate) struct ActionLimits {
    display: Option<Display>,
    scroll_amount: f64,
    max_scroll_amount: f64,
    max_duration: Duration,
    max_repeat: u32,
}

impl ActionLimits {
    pub(crate) fn read(prompt: &Prompt, machine: bool) -> Result<ActionLimits> {
        let side =
            |name| -> Result<u32> { Ok(u32::try_from(prompt.limit(name)?).unwrap_or(u32::MAX)) };
        let display = match machine {
            true => Some(Display::new(side("width")?, side("height")?)),
            false => None,
        };
        Ok(ActionLimits {
            display,
            scroll_amount: float_limit(prompt, "turns")?,
            max_scroll_amount: float_limit(prompt, "most_turns")?,
            max_duration: Duration::from_secs(prompt.limit("longest_seconds")?),
            max_repeat: side("most_repeats")?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct Call {
    actions: Vec<Args>,
}

#[derive(Debug, Deserialize)]
struct Args {
    action: String,
    #[serde(default)]
    coordinate: Option<[i64; 2]>,
    #[serde(default)]
    start_coordinate: Option<[i64; 2]>,
    #[serde(default)]
    region: Option<[i64; 4]>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    repeat: Option<u32>,
    #[serde(default)]
    scroll_direction: Option<String>,
    #[serde(default)]
    scroll_amount: Option<f64>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    desktop: Option<bool>,
}

#[derive(Debug)]
pub(crate) struct Step {
    pub(crate) name: String,
    pub(crate) action: Action,
}

#[derive(Debug)]
pub(crate) enum Action {
    Start {
        display: Display,
        shell: Shell,
    },
    Close,
    Screenshot,
    Zoom(Region),
    Windows,
    CursorPosition,
    MoveTo(Point),
    Click(Click),
    MouseDown(Click),
    MouseUp(Click),
    Drag {
        from: Option<Point>,
        to: Point,
        modifiers: Modifiers,
    },
    Scroll {
        direction: ScrollDirection,
        amount: f64,
        modifiers: Modifiers,
        at: Option<Point>,
    },
    Key {
        chord: Chord,
        repeat: u32,
    },
    HoldKey {
        chord: Chord,
        duration: Duration,
    },
    Type(String),
    Wait(Duration),
    Launch(String),
}

#[derive(Debug)]
pub(crate) struct Click {
    pub(crate) button: PointerButton,
    pub(crate) count: u32,
    pub(crate) modifiers: Modifiers,
    pub(crate) at: Option<Point>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    Observes,
    Reports,
    Changes,
    Opens,
}

pub(crate) fn parse(input: Value, limits: ActionLimits) -> std::result::Result<Vec<Step>, String> {
    let call: Call =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;
    if call.actions.is_empty() {
        return Err("no actions given".to_owned());
    }
    call.actions
        .into_iter()
        .enumerate()
        .map(|(index, args)| {
            let name = args.action.clone();
            Action::parse(args, limits)
                .map(|action| Step {
                    name: name.clone(),
                    action,
                })
                .map_err(|error| format!("action {} ({name}): {error}", index + 1))
        })
        .collect()
}

impl Action {
    fn parse(args: Args, limits: ActionLimits) -> std::result::Result<Action, String> {
        let Args {
            action,
            coordinate,
            start_coordinate,
            region,
            text,
            repeat,
            scroll_direction,
            scroll_amount,
            duration,
            width,
            height,
            desktop,
        } = args;

        let clamp_duration = |seconds: f64| {
            Duration::from_secs_f64(seconds.clamp(0.0, limits.max_duration.as_secs_f64()))
        };
        let required_duration = || {
            duration
                .map(clamp_duration)
                .ok_or_else(|| missing_parameter("duration"))
        };
        let required_text = || {
            text.as_deref()
                .filter(|text| !text.is_empty())
                .ok_or_else(|| missing_parameter("text"))
        };
        let at = || parse_point(coordinate);
        let required_point =
            || parse_point(coordinate)?.ok_or_else(|| missing_parameter("coordinate"));
        let modifiers = || parse_modifiers(text.as_deref());
        let click = |button, count| -> std::result::Result<Click, String> {
            Ok(Click {
                button,
                count,
                modifiers: modifiers()?,
                at: at()?,
            })
        };

        match action.as_str() {
            "start" => Ok(Action::Start {
                display: match (width, height) {
                    (Some(width), Some(height)) => Display::new(width, height),
                    _ => limits.display.ok_or_else(|| unknown_action(&action))?,
                },
                shell: match desktop.unwrap_or(true) {
                    true => Shell::Enabled,
                    false => Shell::Disabled,
                },
            }),
            "close" if limits.display.is_some() => Ok(Action::Close),
            "screenshot" => Ok(Action::Screenshot),
            "zoom" => Ok(Action::Zoom(parse_region(region)?)),
            "windows" => Ok(Action::Windows),
            "cursor_position" => Ok(Action::CursorPosition),

            "mouse_move" => Ok(Action::MoveTo(required_point()?)),
            "left_click" => Ok(Action::Click(click(PointerButton::Primary, 1)?)),
            "right_click" => Ok(Action::Click(click(PointerButton::Secondary, 1)?)),
            "middle_click" => Ok(Action::Click(click(PointerButton::Middle, 1)?)),
            "double_click" => Ok(Action::Click(click(PointerButton::Primary, 2)?)),
            "triple_click" => Ok(Action::Click(click(PointerButton::Primary, 3)?)),
            "left_mouse_down" => Ok(Action::MouseDown(click(PointerButton::Primary, 1)?)),
            "left_mouse_up" => Ok(Action::MouseUp(click(PointerButton::Primary, 1)?)),
            "left_click_drag" => Ok(Action::Drag {
                from: parse_point(start_coordinate)?,
                to: required_point()?,
                modifiers: modifiers()?,
            }),

            "scroll" => {
                let direction = scroll_direction
                    .as_deref()
                    .ok_or_else(|| missing_parameter("scroll_direction"))?;
                Ok(Action::Scroll {
                    direction: direction.parse::<ScrollDirection>().map_err(|_| {
                        format!(
                            "invalid scroll direction \"{direction}\", expected up, down, left or right"
                        )
                    })?,
                    amount: scroll_amount
                        .unwrap_or(limits.scroll_amount)
                        .abs()
                        .min(limits.max_scroll_amount),
                    modifiers: modifiers()?,
                    at: at()?,
                })
            }

            "key" => Ok(Action::Key {
                chord: parse_chord(required_text()?)?,
                repeat: repeat.unwrap_or(1).clamp(1, limits.max_repeat),
            }),
            "hold_key" => Ok(Action::HoldKey {
                chord: parse_chord(required_text()?)?,
                duration: required_duration()?,
            }),
            "type" => Ok(Action::Type(required_text()?.to_owned())),
            "wait" => Ok(Action::Wait(required_duration()?)),
            "launch" => Ok(Action::Launch(required_text()?.to_owned())),

            _ => Err(unknown_action(&action)),
        }
    }

    pub(crate) fn effect(&self) -> Effect {
        match self {
            Action::Screenshot | Action::Zoom(_) => Effect::Observes,
            Action::Close | Action::Windows | Action::CursorPosition => Effect::Reports,
            Action::Launch(_) => Effect::Opens,
            Action::Start { .. }
            | Action::MoveTo(_)
            | Action::Click(_)
            | Action::MouseDown(_)
            | Action::MouseUp(_)
            | Action::Drag { .. }
            | Action::Scroll { .. }
            | Action::Key { .. }
            | Action::HoldKey { .. }
            | Action::Type(_)
            | Action::Wait(_) => Effect::Changes,
        }
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "prompt limits are small integers"
)]
fn float_limit(prompt: &Prompt, name: &str) -> Result<f64> {
    Ok(prompt.limit(name)? as f64)
}

fn parse_point(coordinate: Option<[i64; 2]>) -> std::result::Result<Option<Point>, String> {
    let Some([x, y]) = coordinate else {
        return Ok(None);
    };
    match (u32::try_from(x), u32::try_from(y)) {
        (Ok(x), Ok(y)) => Ok(Some((x, y))),
        _ => Err(format!("invalid coordinate ({x}, {y})")),
    }
}

fn parse_region(region: Option<[i64; 4]>) -> std::result::Result<Region, String> {
    let [x0, y0, x1, y1] = region.ok_or_else(|| missing_parameter("region"))?;
    let invalid = || format!("invalid region [{x0}, {y0}, {x1}, {y1}]");
    let corner = |x: i64, y: i64| -> std::result::Result<Point, String> {
        Ok((
            u32::try_from(x).map_err(|_| invalid())?,
            u32::try_from(y).map_err(|_| invalid())?,
        ))
    };
    Region::from_corners(corner(x0, y0)?, corner(x1, y1)?).ok_or_else(invalid)
}

fn parse_modifiers(text: Option<&str>) -> std::result::Result<Modifiers, String> {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(Modifiers::NONE);
    };
    rikka_input::parse_modifiers(&strip_underscores(text))
        .map_err(|error| format!("invalid modifiers \"{text}\": {error}"))
}

fn parse_chord(text: &str) -> std::result::Result<Chord, String> {
    strip_underscores(text)
        .parse()
        .map_err(|error| format!("invalid key combination \"{text}\": {error}"))
}

fn strip_underscores(text: &str) -> String {
    text.replace('_', "")
}

fn missing_parameter(parameter: &str) -> String {
    format!("requires {parameter}")
}

fn unknown_action(action: &str) -> String {
    format!("unknown action \"{action}\"")
}
