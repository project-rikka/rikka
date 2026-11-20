use std::ops::Range;

use rikka_input::DeviceEvent;
use rikka_types::Rect;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::zwp_text_input_v3::{self, ContentHint, ContentPurpose, ZwpTextInputV3};

use crate::seat_event::SeatEvent;

#[derive(Debug, Default)]
struct Pending {
    preedit: Option<(String, Option<Range<usize>>)>,
    commit: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CursorRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl CursorRect {
    fn from_rect(rect: Rect) -> CursorRect {
        CursorRect {
            x: rect.x().round() as i32,
            y: rect.y().round() as i32,
            width: rect.width().round().max(1.0) as i32,
            height: rect.height().round().max(1.0) as i32,
        }
    }
}

#[derive(Debug)]
pub(crate) struct TextInput {
    pub(crate) input: ZwpTextInputV3,
    pub(crate) focus: Option<WlSurface>,
    enabled: bool,
    cursor_rect: Option<CursorRect>,
    composing: bool,
    pending: Pending,
}

impl TextInput {
    pub(crate) fn new(input: ZwpTextInputV3) -> TextInput {
        TextInput {
            input,
            focus: None,
            enabled: false,
            cursor_rect: None,
            composing: false,
            pending: Pending::default(),
        }
    }

    pub(crate) fn release(self) {
        self.input.destroy();
    }

    pub(crate) fn set_cursor_rect(&mut self, caret: Option<Rect>) {
        match (self.enabled, caret.map(CursorRect::from_rect)) {
            (false, Some(cursor_rect)) => {
                self.input.enable();
                self.input
                    .set_content_type(ContentHint::empty(), ContentPurpose::Normal);
                self.enabled = true;
                self.send_cursor_rect(cursor_rect);
            }
            (true, Some(cursor_rect)) if self.cursor_rect != Some(cursor_rect) => {
                self.send_cursor_rect(cursor_rect)
            }
            (true, None) => {
                self.input.disable();
                self.input.commit();
                self.enabled = false;
                self.cursor_rect = None;
            }
            _ => {}
        }
    }

    fn send_cursor_rect(&mut self, cursor_rect: CursorRect) {
        self.cursor_rect = Some(cursor_rect);
        self.input.set_cursor_rectangle(
            cursor_rect.x,
            cursor_rect.y,
            cursor_rect.width,
            cursor_rect.height,
        );
        self.input.commit();
    }

    pub(crate) fn handle(&mut self, event: zwp_text_input_v3::Event, out: &mut Vec<SeatEvent>) {
        match event {
            zwp_text_input_v3::Event::Enter { surface } => {
                self.focus = Some(surface);
                self.enabled = false;
                self.cursor_rect = None;
            }
            zwp_text_input_v3::Event::Leave { surface } => {
                if self.enabled {
                    self.input.disable();
                    self.input.commit();
                }
                if std::mem::take(&mut self.composing) {
                    out.push(SeatEvent::Input {
                        surface,
                        event: DeviceEvent::Preedit {
                            text: String::new(),
                            cursor: None,
                        },
                    });
                }
                self.focus = None;
                self.enabled = false;
                self.cursor_rect = None;
                self.pending = Pending::default();
            }
            zwp_text_input_v3::Event::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                let text = text.unwrap_or_default();
                let cursor = span(&text, cursor_begin, cursor_end);
                self.pending.preedit = Some((text, cursor));
            }
            zwp_text_input_v3::Event::CommitString { text } => {
                self.pending.commit = text.filter(|text| !text.is_empty());
            }
            zwp_text_input_v3::Event::Done { .. } => self.apply_done(out),
            _ => {}
        }
    }

    fn apply_done(&mut self, out: &mut Vec<SeatEvent>) {
        let pending = std::mem::take(&mut self.pending);
        let Some(surface) = self.focus.clone() else {
            return;
        };
        if let Some(text) = pending.commit {
            out.push(SeatEvent::Input {
                surface: surface.clone(),
                event: DeviceEvent::Commit(text),
            });
        }
        let (text, cursor) = pending.preedit.unwrap_or_default();
        self.composing = !text.is_empty();
        out.push(SeatEvent::Input {
            surface,
            event: DeviceEvent::Preedit { text, cursor },
        });
    }
}

fn span(text: &str, begin: i32, end: i32) -> Option<Range<usize>> {
    let bounded = |at: i32| {
        let at = usize::try_from(at).ok()?;
        text.is_char_boundary(at).then_some(at)
    };
    let (begin, end) = (bounded(begin)?, bounded(end)?);
    Some(begin.min(end)..begin.max(end))
}
