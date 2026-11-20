use std::rc::Rc;

use ade_git::{Branch, Kind};
use rikka_app::prelude::*;

use crate::project::git::at_git_given;
use crate::project::git::state::Git;

pub(super) fn view(git: &Git) -> impl View {
    let branches: Rc<[Branch]> = git.branches().into();
    let at = branches.iter().position(Branch::is_head);

    combo(at)
        .placeholder(tr!("ade.git.detached"))
        .control_size(ControlSize::Small)
        .enabled(!git.is_busy() && !branches.is_empty())
        .options(branches.iter().map(named))
        .on_select(at_git_given(move |git, index: usize| {
            match branches.get(index) {
                Some(branch) => git.switch(branch),
                None => skip_view(),
            }
        }))
        .fill_width()
}

fn named(branch: &Branch) -> String {
    let name = branch.name().to_owned();
    match (branch.kind(), branch.is_checked_out_elsewhere()) {
        (_, true) => tr!("ade.git.held", branch = &name).to_string(),
        (Kind::Remote, false) => tr!("ade.git.remote", branch = &name).to_string(),
        (Kind::Local, false) => name,
    }
}
