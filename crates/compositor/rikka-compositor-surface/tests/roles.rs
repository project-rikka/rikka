use crate::common::{Server, State};
use rikka_compositor_server::Client;
use rikka_compositor_surface::{
    BufferChange, Committed, PreCommit, Rejected, Role, RoleError, SurfaceId, assign_role,
};
use rikka_wayland_client::wl_subcompositor;

#[derive(Debug, PartialEq, Eq)]
struct Cursor {
    hotspot: (i32, i32),
}

impl Role<State> for Cursor {
    const NAME: &'static str = "cursor";

    fn committed(state: &mut State, commit: &Committed) {
        state.note(format!("cursor committed {:?}", commit.buffer));
    }

    fn destroyed(state: &mut State, _surface: SurfaceId, role: Self) {
        state.note(format!("cursor destroyed at {:?}", role.hotspot));
    }
}

#[derive(Debug)]
struct Toplevel;

impl Role<State> for Toplevel {
    const NAME: &'static str = "xdg_toplevel";

    fn pre_commit(state: &mut State, commit: &PreCommit) -> Result<(), Rejected> {
        if commit.buffer != BufferChange::Attached {
            return Ok(());
        }
        let surface = state
            .surfaces
            .get(commit.surface)
            .unwrap()
            .resource()
            .clone();
        if let Some(client) = Client::from_resource(&surface) {
            client.kill("a buffer before the first configure");
        }
        state.note("rejected an unconfigured buffer");
        Err(Rejected)
    }
}

#[test]
fn surface_role_is_permanent() {
    let server = Server::start();
    let mut client = server.connect();
    let compositor = client.compositor();
    let surface = client.surface(&compositor);
    client.roundtrip();
    let id = surface.id();
    let outcomes = server.call(move |state| {
        let surface = state.surface(id);
        let first = assign_role(state, surface, Cursor { hotspot: (1, 2) });
        let again = assign_role(state, surface, Cursor { hotspot: (3, 4) });
        let other = assign_role(state, surface, Toplevel);
        let name = state.surfaces.get(surface).unwrap().role();
        let ended = state.surfaces.end_role::<Cursor>(surface);
        let reassigned = assign_role(state, surface, Cursor { hotspot: (5, 6) });
        let hotspot = state
            .surfaces
            .role::<Cursor>(surface)
            .map(|cursor| cursor.hotspot);
        (first, again, other, name, ended, reassigned, hotspot)
    });
    assert_eq!(outcomes.0, Ok(()));
    assert_eq!(outcomes.1, Err(RoleError::Active { role: "cursor" }));
    assert_eq!(
        outcomes.2,
        Err(RoleError::Taken {
            current: "cursor",
            requested: "xdg_toplevel"
        })
    );
    assert_eq!(outcomes.3, Some("cursor"));
    assert_eq!(outcomes.4, Some(Cursor { hotspot: (1, 2) }));
    assert_eq!(outcomes.5, Ok(()));
    assert_eq!(outcomes.6, Some((5, 6)));

    let subcompositor: wl_subcompositor::WlSubcompositor = client.global(1);
    let parent = client.surface(&compositor);
    subcompositor.get_subsurface(&surface, &parent);
    assert!(client.fails());
    server.expect("protocol error 0 on wl_subcompositor");
}
