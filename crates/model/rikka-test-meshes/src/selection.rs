use rikka_mesh_edit::{EdgeId, EditMesh, Element, FaceId, SelectAction, SelectMode, VertId};

use crate::rng::Rng;

const PARTIAL: f32 = 0.35;
const HIDDEN: f32 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amount {
    Empty,
    Single,
    Partial,
    All,
}

impl Amount {
    pub fn pick(rng: &mut Rng) -> Amount {
        [Amount::Empty, Amount::Single, Amount::Partial, Amount::All][rng.below(4)]
    }

    pub fn choose<T: Copy>(&self, items: &[T], rng: &mut Rng) -> Vec<T> {
        match self {
            Amount::Empty => Vec::new(),
            Amount::Single if items.is_empty() => Vec::new(),
            Amount::Single => vec![items[rng.below(items.len())]],
            Amount::Partial => items
                .iter()
                .copied()
                .filter(|_| rng.chance(PARTIAL))
                .collect(),
            Amount::All => items.to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub mode: SelectMode,
    pub amount: Amount,
    pub hidden: bool,
}

impl Selection {
    #[must_use]
    pub fn every() -> Vec<Selection> {
        let mut selections = Vec::new();
        for mode in [SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE] {
            for amount in [Amount::Empty, Amount::Single, Amount::Partial, Amount::All] {
                for hidden in [false, true] {
                    selections.push(Selection {
                        mode,
                        amount,
                        hidden,
                    });
                }
            }
        }
        selections
    }

    pub fn random(rng: &mut Rng) -> Selection {
        let every = Selection::every();
        every[rng.below(every.len())]
    }

    pub fn apply(&self, mesh: &mut EditMesh, rng: &mut Rng) {
        mesh.set_select_mode(self.mode);
        mesh.select_all(SelectAction::Deselect);
        if self.hidden {
            match mesh.face_count() {
                0 => {
                    mesh.set_select_mode(SelectMode::VERT);
                    for vert in mesh.verts().collect::<Vec<VertId>>() {
                        if rng.chance(HIDDEN) {
                            mesh.select_vert(vert, true);
                        }
                    }
                }
                _ => {
                    mesh.set_select_mode(SelectMode::FACE);
                    for face in mesh.faces().collect::<Vec<FaceId>>() {
                        if rng.chance(HIDDEN) {
                            mesh.select_face(face, true);
                        }
                    }
                }
            }
            mesh.hide_selected(false);
            mesh.set_select_mode(self.mode);
            mesh.select_all(SelectAction::Deselect);
        }
        let elements = visible(mesh, self.mode);
        let chosen: Vec<Element> = match self.amount {
            Amount::Empty => Vec::new(),
            Amount::Single if elements.is_empty() => Vec::new(),
            Amount::Single => vec![elements[rng.below(elements.len())]],
            Amount::Partial => {
                let mut chosen: Vec<Element> = elements
                    .iter()
                    .copied()
                    .filter(|_| rng.chance(PARTIAL))
                    .collect();
                if chosen.is_empty() && !elements.is_empty() {
                    chosen.push(elements[rng.below(elements.len())]);
                }
                chosen
            }
            Amount::All => elements,
        };
        for &element in &chosen {
            mesh.select_element(element, true);
        }
        mesh.select_mode_flush();
        for &element in chosen.iter().rev().take(2).rev() {
            if mesh.is_element_selected(element) {
                mesh.history_store(element);
            }
        }
        if let Some(&Element::Face(face)) = chosen.last() {
            mesh.set_active_face(Some(face));
        }
    }
}

fn visible(mesh: &EditMesh, mode: SelectMode) -> Vec<Element> {
    if mode.contains(SelectMode::VERT) {
        mesh.verts()
            .filter(|&vert| !mesh.is_vert_hidden(vert))
            .map(Element::Vert)
            .collect()
    } else if mode.contains(SelectMode::EDGE) {
        mesh.edges()
            .filter(|&edge: &EdgeId| !mesh.is_edge_hidden(edge))
            .map(Element::Edge)
            .collect()
    } else {
        mesh.faces()
            .filter(|&face| !mesh.is_face_hidden(face))
            .map(Element::Face)
            .collect()
    }
}
