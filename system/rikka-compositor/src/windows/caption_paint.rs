use rikka_compositor_caption::{Button, Caption};
use rikka_compositor_desktop::WindowId;
use rikka_types::Color;

use super::Windows;
use crate::policy::CaptionLook;
use crate::render::{ButtonPaint, CaptionPaint};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ButtonLight {
    pub(crate) amount: f32,
    pub(crate) pressed: bool,
}

impl Windows {
    pub(crate) fn caption_of(&self, window: WindowId) -> Option<Caption> {
        let record = self.desktop.window(window)?;
        let frame = record.frame();
        if frame.title == 0 {
            return None;
        }
        let policy = self.desktop.policy(window)?;
        let decorated = policy.decorated.as_ref()?;
        Some(
            Caption::new(record.geometry().size)
                .with_reach(frame.reach)
                .with_resizable(decorated.resizable)
                .with_maximized(policy.mode.enlargements().maximized),
        )
    }

    pub(crate) fn caption_paint(
        &self,
        window: WindowId,
        look: CaptionLook,
        focus: f32,
        light: impl Fn(Button) -> ButtonLight,
    ) -> Option<CaptionPaint> {
        let caption = self.caption_of(window)?;
        let geometry = self.desktop.window(window)?.geometry();
        let title = self
            .desktop
            .policy(window)?
            .decorated
            .as_ref()?
            .title
            .clone();
        let title_colour = look.title_unfocused.mix(look.title, focus);
        let buttons = caption
            .buttons()
            .map(|(button, _)| button_paint(look, button, title_colour, light(button)))
            .collect();
        Some(CaptionPaint {
            caption,
            geometry,
            title,
            title_colour,
            title_size: look.title_size,
            fill: look.fill,
            separator: look.separator,
            buttons,
        })
    }
}

fn button_paint(
    look: CaptionLook,
    button: Button,
    resting: Color,
    light: ButtonLight,
) -> ButtonPaint {
    let (fill, glyph) = match (button, light.pressed) {
        (Button::Close, true) => (
            look.danger_pressed,
            resting.mix(look.on_danger, light.amount),
        ),
        (Button::Close, false) => (look.danger, resting.mix(look.on_danger, light.amount)),
        (Button::Minimize | Button::Maximize | Button::Restore, true) => (look.press, resting),
        (Button::Minimize | Button::Maximize | Button::Restore, false) => (look.hover, resting),
    };
    ButtonPaint {
        button,
        fill: fill.scale_alpha(light.amount),
        glyph,
    }
}
