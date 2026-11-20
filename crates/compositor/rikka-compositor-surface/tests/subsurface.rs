use crate::common::{Server, Wayland};
use rikka_compositor_surface::SurfaceId;
use rikka_test_display::Canvas;
use rikka_wayland_client::wl_shm::Format;
use rikka_wayland_client::{wl_compositor, wl_subcompositor, wl_subsurface, wl_surface};

struct Scene {
    client: Wayland,
    compositor: wl_compositor::WlCompositor,
    subcompositor: wl_subcompositor::WlSubcompositor,
    buffers: Vec<Canvas>,
}

impl Scene {
    fn new(server: &Server) -> Self {
        let client = server.connect();
        let compositor = client.compositor();
        let subcompositor = client.global(1);
        Self {
            client,
            compositor,
            subcompositor,
            buffers: Vec::new(),
        }
    }

    fn mapped(&mut self, size: i32) -> wl_surface::WlSurface {
        let surface = self.client.surface(&self.compositor);
        let buffer = self.client.canvas(size, size, Format::Argb8888);
        surface.attach(Some(&buffer.buffer), 0, 0);
        self.buffers.push(buffer);
        surface
    }

    fn child(
        &mut self,
        parent: &wl_surface::WlSurface,
        size: i32,
    ) -> (wl_surface::WlSurface, wl_subsurface::WlSubsurface) {
        let surface = self.mapped(size);
        let subsurface = self.subcompositor.get_subsurface(&surface, parent);
        (surface, subsurface)
    }
}

fn ids(server: &Server, surfaces: &[&wl_surface::WlSurface]) -> Vec<SurfaceId> {
    let protocol: Vec<u32> = surfaces.iter().map(|surface| surface.id()).collect();
    server.call(move |state| protocol.iter().map(|id| state.surface(*id)).collect())
}

fn applied(server: &Server) -> Vec<SurfaceId> {
    server.call(|state| {
        state
            .committed
            .drain(..)
            .map(|commit| commit.surface)
            .collect()
    })
}

#[test]
fn synced_children_apply_in_tree_order() {
    let server = Server::start();
    let mut scene = Scene::new(&server);
    let parent = scene.mapped(100);
    parent.commit();
    let (child, _child_role) = scene.child(&parent, 20);
    let (grandchild, grandchild_role) = scene.child(&child, 10);
    grandchild_role.set_desync();
    scene.client.roundtrip();
    let [parent_id, child_id, grandchild_id] = ids(&server, &[&parent, &child, &grandchild])[..]
    else {
        unreachable!()
    };
    assert_eq!(applied(&server), vec![parent_id]);

    grandchild.commit();
    child.commit();
    scene.client.roundtrip();
    assert_eq!(applied(&server), vec![]);

    parent.commit();
    scene.client.roundtrip();
    assert_eq!(applied(&server), vec![parent_id, child_id, grandchild_id]);

    let placed = server.call(move |state| state.surfaces.tree(parent_id).collect::<Vec<_>>());
    assert_eq!(
        placed
            .iter()
            .map(|placed| placed.surface)
            .collect::<Vec<_>>(),
        vec![parent_id, child_id, grandchild_id]
    );
}

#[test]
fn subsurface_cannot_be_own_ancestor() {
    let server = Server::start();
    let mut scene = Scene::new(&server);
    let parent = scene.mapped(100);
    let (child, _role) = scene.child(&parent, 20);
    scene.subcompositor.get_subsurface(&parent, &child);
    assert!(scene.client.fails());
    server.expect("protocol error 1 on wl_subcompositor");
}

#[test]
fn deep_synced_chain_applies_with_root() {
    const DEPTH: usize = 10_000;
    let server = Server::start();
    let mut scene = Scene::new(&server);
    let root = scene.client.surface(&scene.compositor);
    let mut chain = vec![root.clone()];
    let mut roles = Vec::with_capacity(DEPTH);
    for level in 0..DEPTH {
        let child = scene.client.surface(&scene.compositor);
        let parent = chain.last().expect("the chain has a root");
        roles.push(scene.subcompositor.get_subsurface(&child, parent));
        chain.push(child);
        if level % 500 == 0 {
            scene.client.roundtrip();
        }
    }
    for child in chain.iter().skip(1).rev() {
        child.commit();
    }
    scene.client.roundtrip();
    assert!(applied(&server).is_empty());

    root.commit();
    scene.client.roundtrip();
    let order = applied(&server);
    assert_eq!(order.len(), DEPTH + 1);
    let expected = ids(&server, &chain.iter().collect::<Vec<_>>());
    assert_eq!(order, expected);
}
