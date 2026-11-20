use std::sync::Arc;

use cut_project::{Appearance, Art, ClipId, Edit, Graphic, Redraw};
use rikka_app::prelude::*;
use rikka_figure::TextHit;
use rikka_figure_editing::text_with_preedit;

use super::Cut;
use crate::wording;

#[derive(Clone, Debug)]
pub(crate) struct Draft {
    pub(crate) clip: ClipId,
    pub(crate) document: TextDocument,
    pub(crate) typing: bool,
    committed: String,
}

impl Draft {
    fn new(clip: ClipId, text: &str) -> Draft {
        Draft {
            clip,
            document: TextDocument::new(text),
            typing: false,
            committed: text.to_owned(),
        }
    }

    pub(crate) fn same(&self, other: &Draft) -> bool {
        self.clip == other.clip
            && self.typing == other.typing
            && self.document.same(&other.document)
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.document.text() != self.committed
    }

    pub(crate) fn with_draft(&self, graphic: &Graphic) -> Graphic {
        graphic.with_text(text_with_preedit(&self.document))
    }
}

impl Cut {
    pub(crate) fn draft(&self) -> Option<&Draft> {
        self.session.writing.as_ref()
    }

    fn written_clip(&self) -> Option<(ClipId, String)> {
        let (id, graphic) = self.selected_graphic()?;
        match graphic.art {
            Art::Text { lettering, .. } => Some((id, lettering.text)),
            Art::Shape { .. } | Art::Line { .. } => None,
        }
    }

    pub(crate) fn sync_writing(&mut self) {
        let Some((id, text)) = self.written_clip() else {
            if self.session.writing.is_some() {
                self.commit_text();
                self.session.writing = None;
            }
            return;
        };
        match &mut self.session.writing {
            Some(draft) if draft.clip == id => {
                if draft.committed != text {
                    let typing = draft.typing;
                    *draft = Draft::new(id, &text);
                    draft.typing = typing;
                }
            }
            _ => {
                self.commit_text();
                self.session.writing = Some(Draft::new(id, &text));
            }
        }
    }

    pub(crate) fn commit_text(&mut self) {
        let Some(draft) = &self.session.writing else {
            return;
        };
        let text = draft.document.text();
        if text == draft.committed {
            return;
        }
        let clip = draft.clip;
        let at = self.transport.frame();
        let Some(graphic) = self.project().locate(clip).and_then(|(_, clip_ref)| {
            clip_ref.graphic().map(|graphic| {
                (**graphic)
                    .clone()
                    .with_placement(clip_ref.placement_at(at))
            })
        }) else {
            return;
        };
        if !graphic.is_text() {
            return;
        }
        let rewritten = graphic.with_text(text.clone());
        match self.document.apply(&Edit::Redraw {
            changes: vec![(clip, Appearance::Graphic(rewritten))],
            by: Redraw::Text,
            at,
        }) {
            Ok(_) => {
                if let Some(draft) = &mut self.session.writing {
                    draft.committed = text;
                }
            }
            Err(error) => self.notice = Some(wording::refused(&error)),
        }
    }

    pub(crate) fn type_into(&mut self, clip: ClipId, caret: Option<TextHit>) {
        if !self.session.selected.contains(&clip) || self.session.selected.len() != 1 {
            self.session.selected = Arc::new([clip].into_iter().collect());
        }
        self.sync_writing();
        let Some(draft) = &mut self.session.writing else {
            return;
        };
        draft.typing = true;
        match caret {
            Some(hit) => {
                draft.document.place_cursor(hit.at, hit.affinity, false);
            }
            None => {
                draft.document.select_all();
            }
        }
    }

    pub(crate) fn stop_typing(&mut self) {
        self.commit_text();
        if let Some(draft) = &mut self.session.writing {
            draft.typing = false;
        }
    }
}
