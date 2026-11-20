use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_seat::{
    ConstraintState, ContentHint, ContentPurpose as WireContentPurpose, CursorImage, SeatEvent,
    TextInputState,
};
use rikka_compositor_surface::SurfaceId;
use rikka_types::{CursorShape, Rect};

use super::state::State;
use crate::event::Event;
use crate::look::{Constraint, ContentHints, ContentPurpose, Cursor, Surrounding, TextInput};
use crate::view::ViewId;

#[derive(Debug, Default)]
pub(crate) struct CursorState {
    pub(crate) cursor: Option<(ViewId, SurfaceId, Point<i32, Logical>)>,
}

impl State {
    pub(crate) fn seat_event_arrived(&mut self, event: SeatEvent) {
        match event {
            SeatEvent::Cursor { image, .. } => self.cursor_changed(image),
            SeatEvent::Constraint { surface, state, .. } => {
                let constraint = match state {
                    ConstraintState::Locked => Some(Constraint::Locked),
                    ConstraintState::Confined => Some(Constraint::Confined),
                    ConstraintState::Released => None,
                };
                if let Some(view) = self.view_of(surface) {
                    self.change_look(view, |look| look.constraint = constraint);
                    self.event_sender
                        .send(Event::PointerConstraint { view, constraint });
                }
            }
            SeatEvent::Caret { caret, .. } => self.caret_moved(caret),
            SeatEvent::TextInput { state, .. } => self.text_input_changed(state),
            SeatEvent::IdleInhibitors => self.inhibitors_changed(),
            _ => {}
        }
    }

    pub(crate) fn change_look(
        &mut self,
        view: ViewId,
        change: impl FnOnce(&mut crate::look::Look),
    ) {
        if let Some(record) = self.views.records.get(&view) {
            record.shared.change(change);
        }
    }

    fn cursor_changed(&mut self, image: CursorImage) {
        let Some(view) = self.pointer.view else {
            return;
        };
        self.cursor_state.cursor = None;
        let cursor = match image {
            CursorImage::Hidden => Cursor::Hidden,
            CursorImage::Shape(shape) => {
                Cursor::Shape(CursorShape::from_css(shape.name()).unwrap_or(CursorShape::Default))
            }
            CursorImage::Surface { surface, hotspot } => {
                self.cursor_state.cursor = Some((view, surface, hotspot));
                self.cursor_image(surface, hotspot)
            }
        };
        self.change_look(view, |look| look.cursor = cursor);
    }

    fn cursor_image(&mut self, surface: SurfaceId, hotspot: Point<i32, Logical>) -> Cursor {
        match self.layer(surface, Point::zero()) {
            Some(layer) => Cursor::Image {
                layer,
                hotspot: hotspot.into(),
            },
            None => Cursor::Hidden,
        }
    }

    pub(crate) fn ornament_committed(&mut self, root: SurfaceId) {
        if let Some((view, surface, hotspot)) = self.cursor_state.cursor
            && surface == root
        {
            let cursor = self.cursor_image(surface, hotspot);
            self.change_look(view, |look| look.cursor = cursor);
        }
        self.drag_icon_committed(root);
    }

    pub(crate) fn ornament_destroyed(&mut self, surface: SurfaceId) {
        if let Some((view, cursor, _)) = self.cursor_state.cursor
            && cursor == surface
        {
            self.cursor_state.cursor = None;
            self.change_look(view, |look| look.cursor = Cursor::Hidden);
        }
    }

    fn caret_moved(&mut self, caret: Option<rikka_compositor_seat::Caret>) {
        let Some(caret) = caret else {
            if let Some(view) = self.keyboard.focused {
                self.change_look(view, |look| look.caret = None);
            }
            return;
        };
        let Some(view) = self.view_of(caret.surface) else {
            return;
        };
        let Some(origin) = self
            .views
            .records
            .get(&view)
            .and_then(|record| record.origin_of(caret.surface))
        else {
            return;
        };
        let rect = Rect::new(
            (origin.x + caret.rect.loc.x) as f32,
            (origin.y + caret.rect.loc.y) as f32,
            caret.rect.size.w as f32,
            caret.rect.size.h as f32,
        );
        self.change_look(view, |look| look.caret = Some(rect));
    }

    fn text_input_changed(&mut self, state: Option<TextInputState>) {
        let view = match &state {
            Some(state) => self.view_of(state.surface),
            None => self.keyboard.focused,
        };
        let Some(view) = view else {
            return;
        };
        let text_input = state.map(|state| TextInput {
            purpose: purpose_of(state.purpose),
            hints: hints_of(state.hint),
            surrounding: state.surrounding.map(|surrounding| Surrounding {
                text: surrounding.text,
                cursor: surrounding.cursor as usize,
                anchor: surrounding.anchor as usize,
            }),
        });
        let reported = text_input.clone();
        self.change_look(view, |look| look.text_input = text_input);
        self.event_sender.send(Event::TextInput {
            view,
            state: reported,
        });
    }

    fn inhibitors_changed(&mut self) {
        let inhibitors: Vec<SurfaceId> = self.seats.idle_inhibitors().collect();
        let views: Vec<ViewId> = inhibitors
            .iter()
            .filter_map(|surface| self.view_of(*surface))
            .collect();
        for (view, record) in &self.views.records {
            let inhibits = views.contains(view);
            if record.shared.look().inhibits_idle != inhibits {
                record.shared.change(|look| look.inhibits_idle = inhibits);
            }
        }
        self.event_sender
            .send(Event::IdleInhibited(self.seats.idle_inhibited()));
    }
}

fn purpose_of(purpose: WireContentPurpose) -> ContentPurpose {
    match purpose {
        WireContentPurpose::Normal => ContentPurpose::Normal,
        WireContentPurpose::Alpha => ContentPurpose::Alpha,
        WireContentPurpose::Digits => ContentPurpose::Digits,
        WireContentPurpose::Number => ContentPurpose::Number,
        WireContentPurpose::Phone => ContentPurpose::Phone,
        WireContentPurpose::Url => ContentPurpose::Url,
        WireContentPurpose::Email => ContentPurpose::Email,
        WireContentPurpose::Name => ContentPurpose::Name,
        WireContentPurpose::Password => ContentPurpose::Password,
        WireContentPurpose::Pin => ContentPurpose::Pin,
        WireContentPurpose::Date => ContentPurpose::Date,
        WireContentPurpose::Time => ContentPurpose::Time,
        WireContentPurpose::Datetime => ContentPurpose::Datetime,
        WireContentPurpose::Terminal => ContentPurpose::Terminal,
    }
}

fn hints_of(hint: ContentHint) -> ContentHints {
    ContentHints {
        completion: hint.contains(ContentHint::COMPLETION),
        spellcheck: hint.contains(ContentHint::SPELLCHECK),
        auto_capitalization: hint.contains(ContentHint::AUTO_CAPITALIZATION),
        lowercase: hint.contains(ContentHint::LOWERCASE),
        uppercase: hint.contains(ContentHint::UPPERCASE),
        titlecase: hint.contains(ContentHint::TITLECASE),
        hidden_text: hint.contains(ContentHint::HIDDEN_TEXT),
        sensitive_data: hint.contains(ContentHint::SENSITIVE_DATA),
        latin: hint.contains(ContentHint::LATIN),
        multiline: hint.contains(ContentHint::MULTILINE),
    }
}
