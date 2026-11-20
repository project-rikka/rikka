use rikka_types::{CornerRadii, Rect, RoundedRect, Size};
use rikka_ui::{Length, PaintCtx, Styled, View};
use shell_desk::widgets::Mode;
use shell_desk::{Scene, scene};
use shell_kit::framed;

use crate::editor_canvas::card_radius;
use crate::session::Session;
use crate::stage::Editor;

pub(crate) fn view(editor: &Editor) -> impl View {
    let session = editor.session().clone();
    let shown = editor.shown();
    let (layout, grabbed) = match session.draft() {
        Some(draft) => (draft.layout, draft.grabbed),
        None => (editor.layout_of(shown.as_deref()), None),
    };
    let output = editor.output();
    let painted = scene(Scene {
        wallpaper: shown,
        layout,
        output,
        area: editor.area(),
        mode: Mode::Shown,
        grabbed,
    });
    framed(painted, output, move |bounds, ctx| {
        place(&session, bounds, output, ctx)
    })
    .width(Length::px(output.width))
    .height(Length::px(output.height))
    .position(0.0, 0.0)
}

fn place(session: &Session, bounds: Rect, natural: Size, ctx: &mut PaintCtx<'_>) -> Rect {
    let Some(shot) = session.shot() else {
        return bounds;
    };
    if shot.moving {
        ctx.request_frame();
    }
    let radius = card_radius(shot.spread);
    if radius > 0.0 {
        let factor = shot.card.width() / natural.width.max(1.0);
        ctx.set_children_clip(RoundedRect::new(
            Rect::from_origin_size(shot.card.origin, natural),
            CornerRadii::uniform(radius / factor.max(f32::EPSILON)),
        ));
    }
    shot.card
}
