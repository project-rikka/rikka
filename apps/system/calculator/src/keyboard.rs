use calc_engine::{Engine, Key as Pad, Operator, SEPARATOR, Unary};
use rikka_app::prelude::*;

pub(crate) fn keyboard_input(content: impl View) -> Element {
    Element::with_children(KeyboardInput, vec![content.build()])
}

struct KeyboardInput;

impl Widget for KeyboardInput {
    type State = ContextMenu;

    fn layout(&self) -> Layout {
        Layout::column().size(Length::FILL, Length::FILL)
    }

    fn paint(&self, menu: &mut ContextMenu, ctx: &mut PaintCtx<'_>) {
        menu.paint(ctx);
    }

    fn event(&self, menu: &mut ContextMenu, event: &Event, ctx: &mut EventCtx<'_>) {
        if menu.event(event, ctx) {
            return;
        }
        match *event {
            Event::PointerPressed {
                button: PointerButton::Secondary,
                position,
                ..
            } => menu.open(menu_items(), position, ctx),
            Event::Command(Command::ContextMenu) => menu.open_by_keys(menu_items(), None, ctx),
            Event::KeyPressed { key, modifiers, .. } => {
                let Some(pressed) = pad_key(key, modifiers) else {
                    return;
                };
                ctx.set_handled();
                ctx.run(move |engine: &mut Engine| engine.press(pressed));
            }
            Event::Command(Command::Copy) => {
                ctx.set_handled();
                ctx.run(|engine: &mut Engine| {
                    rikka_ui::clipboard::set_text(&engine.display_ungrouped());
                    skip_view();
                });
            }
            Event::Command(Command::Paste) => {
                ctx.set_handled();
                let Some(text) = rikka_ui::clipboard::text() else {
                    return;
                };
                ctx.run(move |engine: &mut Engine| paste(engine, &text));
            }
            _ => {}
        }
    }

    fn behavior(&self, _menu: &ContextMenu) -> Behavior {
        Behavior::FOCUSABLE | Behavior::NO_TAB_STOP | Behavior::AUTOFOCUS
    }

    fn debug_name(&self) -> &'static str {
        "KeyboardInput"
    }
}

fn menu_items() -> [MenuItem; 2] {
    [
        command_item(Command::Copy),
        command_item(Command::Paste).enabled(rikka_ui::clipboard::has_text()),
    ]
}

fn pad_key(key: Key, modifiers: Modifiers) -> Option<Pad> {
    if modifiers.control || modifiers.alt || modifiers.logo {
        return None;
    }
    Some(match key {
        Key::Char(typed) => return char_key(typed),
        Key::Enter => Pad::Equals,
        Key::Backspace => Pad::Backspace,
        Key::Escape => Pad::Clear,
        Key::Delete => Pad::ClearEntry,
        Key::F(9) => Pad::Unary(Unary::Negate),
        _ => return None,
    })
}

fn char_key(typed: char) -> Option<Pad> {
    Some(match typed {
        '0'..='9' => Pad::Digit(typed as u8 - b'0'),
        '.' | ',' => Pad::Point,
        '+' => Pad::Operator(Operator::Add),
        '-' | '−' => Pad::Operator(Operator::Subtract),
        '*' | '×' => Pad::Operator(Operator::Multiply),
        '/' | '÷' => Pad::Operator(Operator::Divide),
        '=' => Pad::Equals,
        '%' => Pad::Percent,
        'r' | 'R' => Pad::Unary(Unary::Reciprocal),
        'q' | 'Q' => Pad::Unary(Unary::Square),
        '@' | '√' => Pad::Unary(Unary::SquareRoot),
        _ => return None,
    })
}

fn paste(engine: &mut Engine, text: &str) {
    for typed in text.trim().chars() {
        if typed.is_whitespace() || typed == SEPARATOR {
            continue;
        }
        let Some(key) = char_key(typed) else {
            return;
        };
        engine.press(key);
    }
}
