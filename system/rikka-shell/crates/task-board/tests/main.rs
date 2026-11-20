mod arrange;
mod press;

use rikka_session::toplevels::{Bounds, Toplevel};

pub(crate) fn toplevel(id: &str, app_id: &str, workspace: usize) -> Toplevel {
    Toplevel {
        id: id.to_owned(),
        app_id: app_id.to_owned(),
        title: id.to_owned(),
        output: "DP-1".to_owned(),
        workspace,
        focused: false,
        bounds: Bounds::default(),
        on_screen: true,
        minimized: false,
        maximized: false,
        tiled: false,
    }
}

pub(crate) fn by_app(toplevel: &Toplevel) -> Option<String> {
    (!toplevel.app_id.is_empty()).then(|| toplevel.app_id.clone())
}
