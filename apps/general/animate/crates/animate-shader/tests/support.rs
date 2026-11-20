use std::sync::Arc;

use animate_look::{GraphRef, Look, LookEdit, shared_library};
use animate_shader::{Workbench, empty_material};
use rikka_shader_graph::{Library, NodeId, NodeKind};

pub(crate) fn library() -> Arc<Library> {
    Arc::new(shared_library().clone())
}

pub(crate) struct Bench {
    pub(crate) workbench: Workbench,
    pub(crate) look: Look,
}

impl Bench {
    pub(crate) fn new() -> Self {
        let library = library();
        let mut look = Look::new(&library).expect("the default look builds");
        let workbench = Workbench::new(Arc::clone(&library));
        let skin = empty_material(&library).expect("the empty material is valid");
        let (key, edit) = workbench
            .add_material(&look, "Skin", skin)
            .expect("a free key");
        assert_eq!(key, "Skin");
        look.apply(&library, edit).expect("the material inserts");
        let mut bench = Self { workbench, look };
        bench.workbench.sync(&bench.look);
        bench
    }

    pub(crate) fn apply(&mut self, edit: LookEdit) -> LookEdit {
        let library = Arc::clone(self.workbench.library());
        let undo = self.look.apply(&library, edit).expect("the edit applies");
        self.workbench.sync(&self.look);
        undo
    }
}

pub(crate) fn skin() -> GraphRef {
    GraphRef::Material("Skin".to_owned())
}

pub(crate) fn find(look: &Look, at: &GraphRef, wanted: fn(&NodeKind) -> bool) -> NodeId {
    *look
        .graph(at)
        .expect("the graph exists")
        .body()
        .nodes()
        .iter()
        .find(|(_, node)| wanted(&node.kind))
        .expect("the node exists")
        .0
}
