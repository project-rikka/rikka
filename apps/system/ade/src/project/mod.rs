mod act;
mod assembly;
mod attaching;
mod catalogs;
mod chat;
mod composer;
mod explorer;
mod file;
mod git;
mod glyph;
mod handle;
mod hub;
mod live;
mod machine;
mod machine_input;
mod palette;
mod places;
mod runner;
mod runners;
mod sidebar;
mod state;
mod tab;
mod terminal;
mod terminals;
mod threads;
mod tool_call;
mod view;
mod workspace;
mod worktree;

use std::time::Duration;

use rikka_app::prelude::*;

pub(crate) const QUIET: Duration = Duration::from_millis(400);

pub(crate) const ARRIVAL: Curve = Curve::Tween {
    easing: Easing::EmphasizedDecel,
    duration: DurationToken::Short4,
};

pub(crate) use handle::ProjectHandle;
pub(crate) use hub::{hub, titled};
pub(crate) use state::Project;
pub(crate) use view::{heading, place, view};
