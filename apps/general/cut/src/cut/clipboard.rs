use cut_project::{ClipId, Edit, EditPoint, Piece};
use rikka_app::prelude::*;

use super::{Cut, Pointing};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Clipboard {
    pieces: Vec<Piece>,
    layer: usize,
}

impl Clipboard {
    pub(crate) fn single(&self) -> Option<&Piece> {
        match &self.pieces[..] {
            [piece] => Some(piece),
            _ => None,
        }
    }
}

impl Cut {
    pub(super) fn selected_list(&self) -> Vec<ClipId> {
        self.session.selected.iter().copied().collect()
    }

    fn keep_on_clipboard(&mut self, clips: &[ClipId]) -> bool {
        let project = self.project();
        let pieces = project.pieces(clips);
        let Some(layer) = clips
            .iter()
            .filter_map(|id| project.locate(*id))
            .map(|(layer, _)| layer)
            .min()
        else {
            return false;
        };
        self.clipboard = Some(Clipboard { pieces, layer });
        self.session.keyframing.forget_copied();
        true
    }

    pub(crate) fn can_paste(&self) -> bool {
        self.clipboard.is_some()
    }

    pub(crate) fn copy(&mut self) {
        let clips = self.selected_list();
        if !self.keep_on_clipboard(&clips) {
            skip_view();
        }
    }

    pub(crate) fn cut_selected(&mut self) {
        let clips = self.project().editable(&self.selected_list());
        if self.keep_on_clipboard(&clips) {
            self.edit(Edit::Cut(clips));
        }
    }

    pub(crate) fn paste(&mut self) {
        let Some(layer) = self.clipboard.as_ref().map(|clipboard| clipboard.layer) else {
            return;
        };
        self.paste_at(self.transport.frame(), layer);
    }

    pub(crate) fn paste_at(&mut self, at: i64, layer: usize) {
        let Some(pieces) = self
            .clipboard
            .as_ref()
            .map(|clipboard| clipboard.pieces.clone())
        else {
            return;
        };
        self.edit(Edit::Paste { pieces, at, layer });
    }

    pub(crate) fn open_menu(
        &mut self,
        at: Point,
        clip: Option<ClipId>,
        edit_point: Option<EditPoint>,
        frame: i64,
        layer: usize,
    ) {
        match (edit_point, clip) {
            (Some(edit_point), _) => self.select_edit_point(edit_point),
            (None, Some(clip)) if !self.session.selected.contains(&clip) => self.select(vec![clip]),
            _ => {}
        }
        self.session.pointing = Some(Pointing { at, frame, layer });
    }

    pub(crate) fn close_menu(&mut self) {
        self.session.pointing = None;
    }
}
