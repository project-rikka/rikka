use std::collections::BTreeSet;
use std::sync::Arc;

use cut_project::{Clip, ClipId, Edit, Interpolation, KeyPiece, KeyRef, Property};
use rikka_app::prelude::*;

use super::{Cut, Popup};
use crate::easing::Picking;

const FIT_MARGIN: f64 = 0.04;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Plot {
    #[default]
    Value,
    Speed,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Keyframing {
    open: bool,
    graph: bool,
    plot: Plot,
    picked: Arc<BTreeSet<KeyRef>>,
    plotted: Arc<BTreeSet<Property>>,
    pin: Option<Point>,
    menu: bool,
    copied: Option<KeyPiece>,
}

impl Keyframing {
    pub(crate) fn forget_copied(&mut self) {
        self.copied = None;
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn is_graph(&self) -> bool {
        self.graph
    }

    pub(crate) fn plot(&self) -> Plot {
        self.plot
    }

    pub(crate) fn plotted(&self) -> &Arc<BTreeSet<Property>> {
        &self.plotted
    }

    pub(crate) fn pin(&self) -> Option<Point> {
        self.pin
    }

    pub(crate) fn menu(&self) -> Option<Point> {
        self.pin.filter(|_| self.menu)
    }

    fn forget(&mut self) {
        if !self.picked.is_empty() {
            self.picked = Arc::default();
        }
        if !self.plotted.is_empty() {
            self.plotted = Arc::default();
        }
        self.pin = None;
        self.menu = false;
    }
}

impl Cut {
    pub(crate) fn keyframing(&self) -> &Keyframing {
        &self.session.keyframing
    }

    pub(crate) fn opened_clip(&self) -> Option<&Clip> {
        self.selected_animated()
            .filter(|_| self.session.keyframing.open)
    }

    pub(crate) fn picked_keys(&self) -> Arc<BTreeSet<KeyRef>> {
        let Some(clip) = self.opened_clip() else {
            return Arc::default();
        };
        let picked = &self.session.keyframing.picked;
        match picked.iter().all(|key| clip.holds(key)) {
            true => Arc::clone(picked),
            false => Arc::new(
                picked
                    .iter()
                    .filter(|key| clip.holds(key))
                    .cloned()
                    .collect(),
            ),
        }
    }

    pub(crate) fn has_picked_keys(&self) -> bool {
        !self.picked_keys().is_empty()
    }

    pub(crate) fn forget_keys(&mut self) {
        self.session.keyframing.forget();
    }

    pub(crate) fn toggle_key_lanes(&mut self) {
        self.session.keyframing.open = !self.session.keyframing.open;
        self.session.keyframing.menu = false;
    }

    pub(crate) fn toggle_graph(&mut self) {
        self.session.keyframing.graph = !self.session.keyframing.graph;
        self.session.keyframing.open |= self.session.keyframing.graph;
        self.session.keyframing.menu = false;
    }

    pub(crate) fn set_plot(&mut self, plot: Plot) {
        self.session.keyframing.plot = plot;
    }

    pub(crate) fn plot_property(&mut self, property: Property, adding: bool) {
        let mut plotted: BTreeSet<Property> = (*self.session.keyframing.plotted).clone();
        let alone = plotted.len() == 1 && plotted.contains(&property);
        match adding {
            true => {
                if !plotted.remove(&property) {
                    plotted.insert(property);
                }
            }
            false if alone => plotted.clear(),
            false => plotted = BTreeSet::from([property]),
        }
        self.session.keyframing.plotted = Arc::new(plotted);
    }

    pub(crate) fn pick_keys(&mut self, keys: Vec<KeyRef>) {
        let picked: BTreeSet<KeyRef> = keys.into_iter().collect();
        if picked != *self.session.keyframing.picked {
            self.session.keyframing.picked = Arc::new(picked);
        }
    }

    pub(crate) fn fit_keys(&mut self) {
        skip_view();
        if let Some(clip) = self.selected_animated() {
            let margin = clip.length() as f64 * FIT_MARGIN;
            self.scale
                .show(clip.start() as f64 - margin, clip.end() as f64 + margin);
        }
    }

    pub(crate) fn reshape_keys(&mut self, edits: Vec<Edit>, picked: Vec<KeyRef>) {
        self.commit_edits(edits);
        self.pick_keys(picked);
    }

    fn picked_in_clip(&mut self) -> Option<(ClipId, Vec<KeyRef>)> {
        self.session.keyframing.menu = false;
        let keys: Vec<KeyRef> = self.picked_keys().iter().cloned().collect();
        let clip = self.opened_clip().map(Clip::id)?;
        (!keys.is_empty()).then_some((clip, keys))
    }

    pub(crate) fn interpolate_keys(&mut self, interpolation: Interpolation) {
        if let Some((clip, keys)) = self.picked_in_clip() {
            self.ease_keys(clip, &keys, interpolation);
        }
    }

    pub(crate) fn delete_keys(&mut self) {
        if let Some((clip, keys)) = self.picked_in_clip() {
            self.remove_keys(clip, &keys);
        }
        self.session.keyframing.picked = Arc::default();
    }

    pub(crate) fn edit_key_curve(&mut self, anchor: Anchor) {
        if let Some((clip, keys)) = self.picked_in_clip() {
            self.open_popup(Popup::Easing(Picking::keys(clip, keys, anchor)));
        }
    }

    pub(crate) fn copy_keys(&mut self) {
        self.session.keyframing.menu = false;
        let Some(clip) = self.selected_animated().map(Clip::id) else {
            return;
        };
        let keys: Vec<KeyRef> = self.picked_keys().iter().cloned().collect();
        let piece = self.project().copy_keys(clip, &keys);
        if piece.is_empty() {
            return;
        }
        self.session.keyframing.copied = Some(piece);
    }

    pub(crate) fn cut_keys(&mut self) {
        self.copy_keys();
        self.delete_keys();
    }

    pub(crate) fn can_paste_keys(&self) -> bool {
        self.selected_animated().is_some() && self.session.keyframing.copied.is_some()
    }

    pub(crate) fn paste_keys(&mut self) {
        let Some(copied) = self.session.keyframing.copied.clone() else {
            return;
        };
        let Some(clip) = self.selected_animated().map(Clip::id) else {
            return;
        };
        let at = self.transport.frame();
        let pasted = copied
            .keys()
            .into_iter()
            .map(|key| KeyRef::new(key.property, key.at + at))
            .collect();
        self.commit_edits(vec![Edit::PasteKeys {
            clip,
            keys: copied,
            at,
        }]);
        self.pick_keys(pasted);
    }

    pub(crate) fn open_key_menu(&mut self, at: Point, keys: Vec<KeyRef>) {
        let picked = self.picked_keys();
        if !keys.iter().all(|key| picked.contains(key)) {
            self.pick_keys(keys);
        }
        self.session.pointing = None;
        self.session.popup = None;
        self.session.keyframing.pin = Some(at);
        self.session.keyframing.menu = true;
    }

    pub(crate) fn close_key_menu(&mut self) {
        self.session.keyframing.menu = false;
    }
}
