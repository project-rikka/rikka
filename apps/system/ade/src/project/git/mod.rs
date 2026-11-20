mod diff;
mod group;
mod head;
mod history;
mod lines;
mod operation;
mod origin;
mod painted;
mod pane;
mod patch_request;
mod primary;
mod showing;
mod standing;
mod state;
mod subject;
mod transfer;
mod transferring;

pub(crate) use state::Git;

pub(super) use pane::{glyph, ink, label, view as sidebar};

pub(super) use diff::view as change;

pub(super) const SMALL: f32 = 12.0;

fn at_git(
    act: impl Fn(&mut Git) + 'static,
) -> impl Fn(&mut crate::project::state::Project) + 'static {
    crate::project::act::act(move |project| project.git(&act))
}

fn at_git_given<A: 'static>(
    act: impl Fn(&mut Git, A) + 'static,
) -> impl Fn(&mut crate::project::state::Project, A) + 'static {
    crate::project::act::given(move |project, given: A| project.git(|git| act(git, given)))
}
