use crate::core::Ring;
use crate::ids::{EdgeId, Element, FaceId, LoopId, VertId};
use crate::mesh::{EditMesh, Flags, SelectMode};
use crate::parallel;

impl EditMesh {
    #[must_use]
    pub fn select_mode(&self) -> SelectMode {
        self.select_mode
    }

    pub(crate) fn set_vert_selected_raw(&mut self, vert: VertId, selected: bool) {
        if !self.is_vert_hidden(vert) {
            self.mark_vert(vert, Flags::SELECT, selected);
        }
    }

    pub fn select_edge_noflush(&mut self, edge: EdgeId, selected: bool) {
        if !self.is_edge_hidden(edge) {
            self.mark_edge(edge, Flags::SELECT, selected);
        }
        self.touch_selection();
    }

    pub fn select_face_noflush(&mut self, face: FaceId, selected: bool) {
        if !self.is_face_hidden(face) {
            self.mark_face(face, Flags::SELECT, selected);
        }
        self.touch_selection();
    }

    pub fn select_vert(&mut self, vert: VertId, selected: bool) {
        self.set_vert_selected_raw(vert, selected);
        self.touch_selection();
    }

    pub fn select_edge(&mut self, edge: EdgeId, selected: bool) {
        if self.is_edge_hidden(edge) {
            return;
        }
        self.mark_edge(edge, Flags::SELECT, selected);
        let [a, b] = self.edge_verts(edge);
        if selected {
            self.set_vert_selected_raw(a, true);
            self.set_vert_selected_raw(b, true);
        } else if !self.select_mode.contains(SelectMode::VERT) {
            for vert in [a, b] {
                let other_selected = self
                    .vert_edges(vert)
                    .any(|other| other != edge && self.is_edge_selected(other));
                if !other_selected {
                    self.set_vert_selected_raw(vert, false);
                }
            }
        } else {
            self.set_vert_selected_raw(a, false);
            self.set_vert_selected_raw(b, false);
        }
        self.touch_selection();
    }

    pub fn select_face(&mut self, face: FaceId, selected: bool) {
        if self.is_face_hidden(face) {
            return;
        }
        self.mark_face(face, Flags::SELECT, selected);
        let loops: Ring<LoopId> = self.face_loops(face).collect();
        if selected {
            for &lp in &loops {
                self.set_vert_selected_raw(self.loop_vert(lp), true);
                self.select_edge(self.loop_edge(lp), true);
            }
        } else if self.select_mode.contains(SelectMode::VERT) {
            for &lp in &loops {
                self.set_vert_selected_raw(self.loop_vert(lp), false);
                self.select_edge_noflush(self.loop_edge(lp), false);
            }
        } else {
            for &lp in &loops {
                let edge = self.loop_edge(lp);
                let keep = !self.select_mode.contains(SelectMode::EDGE)
                    && self
                        .edge_loops(edge)
                        .any(|other| other != lp && self.is_face_selected(self.loop_face(other)));
                if !keep {
                    self.select_edge_noflush(edge, false);
                }
            }
            for &lp in &loops {
                let vert = self.loop_vert(lp);
                let edge = self.loop_edge(lp);
                let other_selected = self
                    .vert_edges(vert)
                    .any(|other| other != edge && self.is_edge_selected(other));
                if !other_selected {
                    self.set_vert_selected_raw(vert, false);
                }
            }
        }
        self.touch_selection();
    }

    pub fn select_element(&mut self, element: Element, selected: bool) {
        match element {
            Element::Vert(vert) => self.select_vert(vert, selected),
            Element::Edge(edge) => self.select_edge(edge, selected),
            Element::Face(face) => self.select_face(face, selected),
        }
    }

    #[must_use]
    pub fn is_element_selected(&self, element: Element) -> bool {
        match element {
            Element::Vert(vert) => self.is_vert_selected(vert),
            Element::Edge(edge) => self.is_edge_selected(edge),
            Element::Face(face) => self.is_face_selected(face),
        }
    }

    #[must_use]
    pub fn is_element_hidden(&self, element: Element) -> bool {
        match element {
            Element::Vert(vert) => self.is_vert_hidden(vert),
            Element::Edge(edge) => self.is_edge_hidden(edge),
            Element::Face(face) => self.is_face_hidden(face),
        }
    }

    fn edge_verts_selected(&self, edge: EdgeId) -> bool {
        let [a, b] = self.edge_verts(edge);
        self.is_vert_selected(a) && self.is_vert_selected(b)
    }

    fn edges_where(&self, test: impl Fn(EdgeId) -> bool + Sync) -> Vec<EdgeId> {
        parallel::indices_where(self.edges.capacity(), |index| {
            self.edges.is_alive(index as u32) && test(EdgeId(index as u32))
        })
        .into_iter()
        .map(EdgeId)
        .collect()
    }

    fn faces_where(&self, test: impl Fn(FaceId) -> bool + Sync) -> Vec<FaceId> {
        parallel::indices_where(self.faces.capacity(), |index| {
            self.faces.is_alive(index as u32) && test(FaceId(index as u32))
        })
        .into_iter()
        .map(FaceId)
        .collect()
    }

    pub fn select_flush_from_verts(&mut self, selected: bool) {
        if selected {
            let edges = self.edges_where(|edge| {
                !self.is_edge_selected(edge)
                    && !self.is_edge_hidden(edge)
                    && self.edge_verts_selected(edge)
            });
            for edge in edges {
                self.mark_edge(edge, Flags::SELECT, true);
            }
            let faces = self.faces_where(|face| {
                !self.is_face_selected(face)
                    && !self.is_face_hidden(face)
                    && self
                        .face_verts(face)
                        .all(|vert| self.is_vert_selected(vert))
            });
            for face in faces {
                self.mark_face(face, Flags::SELECT, true);
            }
        } else {
            let deselected = |edge: EdgeId| {
                !self.is_edge_hidden(edge)
                    && (!self.is_edge_selected(edge) || !self.edge_verts_selected(edge))
            };
            let faces = self.faces_where(|face| {
                self.is_face_selected(face) && self.face_edges(face).any(deselected)
            });
            let edges = self.edges_where(|edge| {
                self.is_edge_selected(edge)
                    && !self.is_edge_hidden(edge)
                    && !self.edge_verts_selected(edge)
            });
            for edge in edges {
                self.mark_edge(edge, Flags::SELECT, false);
            }
            for face in faces {
                self.mark_face(face, Flags::SELECT, false);
            }
            self.history_validate();
        }
        self.touch_selection();
    }

    pub fn select_mode_flush(&mut self) {
        self.select_mode_flush_with(self.select_mode, false);
    }

    pub fn select_mode_flush_down(&mut self) {
        self.select_mode_flush_with(self.select_mode, true);
    }

    pub fn select_mode_flush_with(&mut self, mode: SelectMode, down: bool) {
        if down {
            if mode.contains(SelectMode::VERT) {
            } else if mode.contains(SelectMode::EDGE) {
                self.flush_edges_to_verts();
            } else if mode.contains(SelectMode::FACE) {
                self.flush_faces_to_verts_and_edges();
            }
        }
        if mode.contains(SelectMode::VERT) {
            let edges = self.edges_where(|edge| {
                let selected = !self.is_edge_hidden(edge) && self.edge_verts_selected(edge);
                selected != self.is_edge_selected(edge)
            });
            for edge in edges {
                let selected = !self.is_edge_selected(edge);
                self.mark_edge(edge, Flags::SELECT, selected);
            }
        }
        if mode.intersects(SelectMode::VERT | SelectMode::EDGE) {
            let faces = self.faces_where(|face| {
                let selected = !self.is_face_hidden(face)
                    && self
                        .face_edges(face)
                        .all(|edge| self.is_edge_selected(edge));
                selected != self.is_face_selected(face)
            });
            for face in faces {
                let selected = !self.is_face_selected(face);
                self.mark_face(face, Flags::SELECT, selected);
            }
        }
        self.history_validate();
        self.touch_selection();
    }

    fn flush_edges_to_verts(&mut self) {
        let edges: Vec<EdgeId> = self
            .edges()
            .filter(|&edge| !self.is_edge_hidden(edge))
            .collect();
        let mut any = false;
        for &edge in &edges {
            if self.is_edge_selected(edge) {
                any = true;
            } else {
                let [a, b] = self.edge_verts(edge);
                self.set_vert_selected_raw(a, false);
                self.set_vert_selected_raw(b, false);
            }
        }
        if any {
            for &edge in &edges {
                if self.is_edge_selected(edge) {
                    let [a, b] = self.edge_verts(edge);
                    self.set_vert_selected_raw(a, true);
                    self.set_vert_selected_raw(b, true);
                }
            }
        }
    }

    fn flush_faces_to_verts_and_edges(&mut self) {
        let faces: Vec<FaceId> = self
            .faces()
            .filter(|&face| !self.is_face_hidden(face))
            .collect();
        let mut any = false;
        for &face in &faces {
            if self.is_face_selected(face) {
                any = true;
                continue;
            }
            for lp in self.face_loops_vec(face) {
                self.set_vert_selected_raw(self.loop_vert(lp), false);
                let edge = self.loop_edge(lp);
                if !self.is_edge_hidden(edge) {
                    self.mark_edge(edge, Flags::SELECT, false);
                }
            }
        }
        if any {
            for &face in &faces {
                if !self.is_face_selected(face) {
                    continue;
                }
                for lp in self.face_loops_vec(face) {
                    self.set_vert_selected_raw(self.loop_vert(lp), true);
                    let edge = self.loop_edge(lp);
                    if !self.is_edge_hidden(edge) {
                        self.mark_edge(edge, Flags::SELECT, true);
                    }
                }
            }
        }
    }

    pub fn set_select_mode(&mut self, mode: SelectMode) {
        self.select_mode = mode;
        self.strip_history();
        let any_selected = self.verts().any(|vert| self.is_vert_selected(vert))
            || self.edges().any(|edge| self.is_edge_selected(edge))
            || self.faces().any(|face| self.is_face_selected(face));
        self.touch_selection();
        if !any_selected {
            return;
        }
        if mode.contains(SelectMode::VERT) {
            if self.verts().any(|vert| self.is_vert_selected(vert)) {
                self.select_flush_from_verts(true);
            }
        } else if mode.contains(SelectMode::EDGE) {
            let verts: Vec<VertId> = self.verts().collect();
            for vert in verts {
                self.set_vert_selected_raw(vert, false);
            }
            let selected: Vec<EdgeId> = self
                .edges()
                .filter(|&edge| self.is_edge_selected(edge))
                .collect();
            if !selected.is_empty() {
                for edge in selected {
                    self.select_edge(edge, true);
                }
                self.select_mode_flush();
            }
        } else if mode.contains(SelectMode::FACE) {
            let edges: Vec<EdgeId> = self.edges().collect();
            for edge in edges {
                self.select_edge(edge, false);
            }
            let selected: Vec<FaceId> = self
                .faces()
                .filter(|&face| self.is_face_selected(face))
                .collect();
            for face in selected {
                self.select_face(face, true);
            }
        }
    }

    pub fn convert_select_mode(&mut self, from: SelectMode, to: SelectMode) {
        let any = |mesh: &EditMesh, mode: SelectMode| match mode {
            SelectMode::VERT => mesh.verts().any(|vert| mesh.is_vert_selected(vert)),
            SelectMode::EDGE => mesh.edges().any(|edge| mesh.is_edge_selected(edge)),
            _ => mesh.faces().any(|face| mesh.is_face_selected(face)),
        };
        if !any(self, from) {
            return;
        }
        match (from, to) {
            (SelectMode::VERT, SelectMode::EDGE) => {
                let touched: Vec<EdgeId> = self
                    .edges()
                    .filter(|&edge| {
                        self.edge_verts(edge)
                            .iter()
                            .any(|&vert| self.is_vert_selected(vert))
                    })
                    .collect();
                for edge in touched {
                    self.select_edge(edge, true);
                }
            }
            (SelectMode::VERT, SelectMode::FACE) => {
                let touched: Vec<FaceId> = self
                    .faces()
                    .filter(|&face| {
                        self.face_verts(face)
                            .any(|vert| self.is_vert_selected(vert))
                    })
                    .collect();
                for face in touched {
                    self.select_face(face, true);
                }
            }
            (SelectMode::EDGE, SelectMode::FACE) => {
                let touched: Vec<FaceId> = self
                    .faces()
                    .filter(|&face| {
                        self.face_edges(face)
                            .any(|edge| self.is_edge_selected(edge))
                    })
                    .collect();
                for face in touched {
                    self.select_face(face, true);
                }
            }
            (SelectMode::EDGE, SelectMode::VERT) => {
                let partial: Vec<VertId> = self
                    .verts()
                    .filter(|&vert| {
                        self.vert_edges(vert)
                            .any(|edge| !self.is_edge_hidden(edge) && !self.is_edge_selected(edge))
                    })
                    .collect();
                for vert in partial {
                    self.set_vert_selected_raw(vert, false);
                }
                self.select_flush_from_verts(false);
            }
            (SelectMode::FACE, SelectMode::EDGE) => {
                let partial: Vec<EdgeId> = self
                    .edges()
                    .filter(|&edge| {
                        self.edge_faces(edge)
                            .any(|face| !self.is_face_hidden(face) && !self.is_face_selected(face))
                    })
                    .collect();
                for edge in partial {
                    self.select_edge(edge, false);
                }
                self.select_flush_from_verts(false);
            }
            (SelectMode::FACE, SelectMode::VERT) => {
                let partial: Vec<VertId> = self
                    .verts()
                    .filter(|&vert| {
                        self.vert_faces(vert)
                            .any(|face| !self.is_face_hidden(face) && !self.is_face_selected(face))
                    })
                    .collect();
                for vert in partial {
                    self.set_vert_selected_raw(vert, false);
                }
                self.select_flush_from_verts(false);
            }
            _ => {}
        }
        self.touch_selection();
    }

    pub fn change_select_mode(&mut self, mode: SelectMode, expand: bool) {
        if expand {
            let highest = [SelectMode::FACE, SelectMode::EDGE, SelectMode::VERT]
                .into_iter()
                .find(|&single| self.select_mode.contains(single))
                .unwrap_or(SelectMode::VERT);
            self.convert_select_mode(highest, mode);
        }
        self.set_select_mode(mode);
    }

    fn strip_history(&mut self) {
        let mode = self.select_mode;
        self.history.retain(|element| match element {
            Element::Vert(_) => mode.contains(SelectMode::VERT),
            Element::Edge(_) => mode.contains(SelectMode::EDGE),
            Element::Face(_) => mode.contains(SelectMode::FACE),
        });
    }

    #[must_use]
    pub fn select_history(&self) -> &[Element] {
        &self.history
    }

    pub fn history_store(&mut self, element: Element) {
        if !self.history.contains(&element) {
            self.history.push(element);
            self.touch_selection();
        }
    }

    pub fn history_remove(&mut self, element: Element) -> bool {
        let before = self.history.len();
        self.history.retain(|stored| *stored != element);
        self.touch_selection();
        before != self.history.len()
    }

    pub fn history_clear(&mut self) {
        self.history.clear();
        self.touch_selection();
    }

    pub fn history_validate(&mut self) {
        let history = std::mem::take(&mut self.history);
        self.history = history
            .into_iter()
            .filter(|&element| self.is_element_selected(element))
            .collect();
    }

    pub fn set_active(&mut self, element: Element) {
        self.history.retain(|stored| *stored != element);
        self.history.push(element);
        if let Element::Face(face) = element {
            self.active_face = Some(face.0);
        }
        self.touch_selection();
    }

    #[must_use]
    pub fn active_face(&self) -> Option<FaceId> {
        self.active_face.map(FaceId)
    }

    pub fn set_active_face(&mut self, face: Option<FaceId>) {
        self.active_face = face
            .filter(|&face| self.contains_face(face))
            .map(|face| face.0);
        self.touch_selection();
    }

    #[must_use]
    pub fn active_element(&self) -> Option<Element> {
        let active_face = self
            .active_face()
            .filter(|&face| self.is_face_selected(face));
        match self.history.last() {
            Some(Element::Face(face)) => Some(Element::Face(active_face.unwrap_or(*face))),
            Some(&element) => Some(element),
            None => active_face.map(Element::Face),
        }
    }

    pub fn hide_vert(&mut self, vert: VertId, hidden: bool) {
        if hidden {
            self.select_vert(vert, false);
        }
        self.mark_vert(vert, Flags::HIDDEN, hidden);
        for edge in self.vert_edges(vert).collect::<Vec<_>>() {
            self.mark_edge(edge, Flags::HIDDEN, hidden);
            for face in self.edge_faces(edge).collect::<Vec<_>>() {
                self.mark_face(face, Flags::HIDDEN, hidden);
            }
        }
        self.touch_selection();
    }

    pub fn hide_edge(&mut self, edge: EdgeId, hidden: bool) {
        if hidden {
            self.select_edge(edge, false);
        }
        for face in self.edge_faces(edge).collect::<Vec<_>>() {
            self.mark_face(face, Flags::HIDDEN, hidden);
        }
        self.mark_edge(edge, Flags::HIDDEN, hidden);
        let [a, b] = self.edge_verts(edge);
        for vert in [a, b] {
            let visible = match hidden {
                true => self
                    .vert_edges(vert)
                    .any(|other| !self.is_edge_hidden(other)),
                false => true,
            };
            self.mark_vert(vert, Flags::HIDDEN, !visible);
        }
        self.touch_selection();
    }

    pub fn hide_face(&mut self, face: FaceId, hidden: bool) {
        if hidden {
            self.select_face(face, false);
        }
        self.mark_face(face, Flags::HIDDEN, hidden);
        let loops = self.face_loops_vec(face);
        if hidden {
            for &lp in &loops {
                let edge = self.loop_edge(lp);
                let visible = self
                    .edge_faces(edge)
                    .any(|other| !self.is_face_hidden(other));
                self.mark_edge(edge, Flags::HIDDEN, !visible);
            }
            for &lp in &loops {
                let vert = self.loop_vert(lp);
                let visible = self
                    .vert_edges(vert)
                    .any(|other| !self.is_edge_hidden(other));
                self.mark_vert(vert, Flags::HIDDEN, !visible);
            }
        } else {
            for &lp in &loops {
                let edge = self.loop_edge(lp);
                self.mark_edge(edge, Flags::HIDDEN, false);
                let vert = self.loop_vert(lp);
                self.mark_vert(vert, Flags::HIDDEN, false);
            }
        }
        self.touch_selection();
    }

    pub fn hide_selected(&mut self, unselected: bool) -> bool {
        let mut changed = false;
        if self.select_mode.contains(SelectMode::VERT) {
            let verts: Vec<VertId> = self.verts().collect();
            for vert in verts {
                if !self.is_vert_hidden(vert) && self.is_vert_selected(vert) != unselected {
                    self.hide_vert(vert, true);
                    changed = true;
                }
            }
        } else if self.select_mode.contains(SelectMode::EDGE) {
            let edges: Vec<EdgeId> = self.edges().collect();
            for edge in edges {
                if !self.is_edge_hidden(edge) && self.is_edge_selected(edge) != unselected {
                    self.hide_edge(edge, true);
                    changed = true;
                }
            }
        } else {
            let faces: Vec<FaceId> = self.faces().collect();
            for face in faces {
                if !self.is_face_hidden(face) && self.is_face_selected(face) != unselected {
                    self.hide_face(face, true);
                    changed = true;
                }
            }
        }
        if unselected {
            let face_mode = !self
                .select_mode
                .intersects(SelectMode::VERT | SelectMode::EDGE);
            if face_mode {
                let wires: Vec<EdgeId> = self
                    .edges()
                    .filter(|&edge| self.is_edge_wire(edge))
                    .collect();
                for edge in wires {
                    if !self.is_edge_hidden(edge) && !self.is_edge_selected(edge) {
                        self.hide_edge(edge, true);
                        changed = true;
                    }
                }
            }
            if !self.select_mode.contains(SelectMode::VERT) {
                let loose: Vec<VertId> = self
                    .verts()
                    .filter(|&vert| self.is_vert_loose(vert))
                    .collect();
                for vert in loose {
                    if !self.is_vert_hidden(vert) && !self.is_vert_selected(vert) {
                        self.hide_vert(vert, true);
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.select_mode_flush();
        }
        changed
    }

    pub fn reveal(&mut self, select: bool) -> bool {
        let verts: Vec<VertId> = self
            .verts()
            .filter(|&vert| self.is_vert_hidden(vert))
            .collect();
        let edges: Vec<EdgeId> = self
            .edges()
            .filter(|&edge| self.is_edge_hidden(edge))
            .collect();
        let faces: Vec<FaceId> = self
            .faces()
            .filter(|&face| self.is_face_hidden(face))
            .collect();
        if verts.is_empty() && edges.is_empty() && faces.is_empty() {
            return false;
        }
        for &vert in &verts {
            self.mark_vert(vert, Flags::HIDDEN, false);
        }
        for &edge in &edges {
            self.mark_edge(edge, Flags::HIDDEN, false);
        }
        for &face in &faces {
            self.mark_face(face, Flags::HIDDEN, false);
        }
        if self.select_mode.contains(SelectMode::VERT) {
            for vert in verts {
                self.select_vert(vert, select);
            }
        }
        if self.select_mode.contains(SelectMode::EDGE) {
            for edge in edges {
                self.select_edge(edge, select);
            }
        }
        if self.select_mode.contains(SelectMode::FACE) {
            for face in faces {
                self.select_face(face, select);
            }
        }
        self.select_mode_flush();
        true
    }
}
