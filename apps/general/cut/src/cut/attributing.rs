use cut_project::{Attribute, ClipId, Edit, KeyFit, Piece};

use super::Cut;
use super::sheet::{Sheet, draft, draft_mut};
use crate::attributes::Pasting;

impl Cut {
    pub(crate) fn pasting(&self) -> Option<&Pasting> {
        draft(&self.sheet)
    }

    fn editable_selected(&self) -> Vec<ClipId> {
        self.project().editable(&self.selected_list())
    }

    fn copied_onto(&self) -> Option<(&Piece, Vec<ClipId>)> {
        let piece = self.clipboard.as_ref()?.single()?;
        let onto = self.editable_selected();
        (!onto.is_empty()).then_some((piece, onto))
    }

    fn pastable(&self) -> Vec<Attribute> {
        self.copied_onto()
            .map(|(piece, onto)| self.project().pastable(piece, &onto))
            .unwrap_or_default()
    }

    pub(crate) fn can_paste_attributes(&self) -> bool {
        !self.pastable().is_empty()
    }

    pub(crate) fn can_paste_effects(&self) -> bool {
        self.pastable().into_iter().any(Attribute::is_effect)
    }

    pub(crate) fn can_remove_effects(&self) -> bool {
        let project = self.project();
        self.editable_selected()
            .into_iter()
            .filter_map(|id| project.locate(id))
            .any(|(_, clip)| !clip.effects().is_empty())
    }

    pub(crate) fn open_pasting(&mut self) {
        if self.is_modal() {
            return;
        }
        let Some((piece, onto)) = self.copied_onto() else {
            return;
        };
        let offered = self.project().pastable(piece, &onto);
        if offered.is_empty() {
            return;
        }
        let pasting = Pasting::new(piece.clone(), onto, offered);
        self.commit_text();
        self.close_marker();
        self.pause();
        self.sheet = Some(Sheet::PasteAttributes(Box::new(pasting)));
    }

    pub(crate) fn close_pasting(&mut self) {
        self.sheet = None;
    }

    pub(crate) fn tune_pasting(&mut self, change: impl FnOnce(&mut Pasting)) {
        if let Some(pasting) = draft_mut(&mut self.sheet) {
            change(pasting);
        }
    }

    pub(crate) fn paste_picked(&mut self) {
        let Some(edit) = draft::<Pasting>(&self.sheet).and_then(Pasting::edit) else {
            return;
        };
        self.sheet = None;
        self.edit(edit);
    }

    pub(crate) fn paste_effects(&mut self) {
        let Some((piece, onto)) = self.copied_onto() else {
            return;
        };
        let attributes: Vec<Attribute> = self
            .project()
            .pastable(piece, &onto)
            .into_iter()
            .filter(|attribute| attribute.is_effect())
            .collect();
        if attributes.is_empty() {
            return;
        }
        let from = Box::new(piece.clone());
        self.edit(Edit::PasteAttributes {
            from,
            onto,
            attributes,
            keys: KeyFit::Maintain,
        });
    }

    pub(crate) fn remove_effects(&mut self) {
        let clips = self.editable_selected();
        if clips.is_empty() {
            return;
        }
        self.session.popup = None;
        self.live.settle();
        self.edit(Edit::RemoveEffects(clips));
    }
}
