use crate::ade::Ade;
use crate::project::state::Project;
use crate::setup::Setup;

pub(crate) fn act(act: impl Fn(&mut Project) + 'static) -> impl Fn(&mut Project) + 'static {
    move |project: &mut Project| {
        act(project);
        project.settle();
    }
}

pub(crate) fn given<A: 'static>(
    act: impl Fn(&mut Project, A) + 'static,
) -> impl Fn(&mut Project, A) + 'static {
    move |project: &mut Project, given: A| {
        act(project, given);
        project.settle();
    }
}

pub(crate) fn with_setup(
    act: impl Fn(&mut Project, &mut Setup) + 'static,
) -> impl Fn(&mut Ade) + 'static {
    move |ade: &mut Ade| {
        ade.work(&act);
    }
}

pub(crate) fn given_with_setup<A: 'static>(
    act: impl Fn(&mut Project, &mut Setup, A) + 'static,
) -> impl Fn(&mut Ade, A) + 'static {
    move |ade: &mut Ade, given: A| {
        ade.work(|project, setup| act(project, setup, given));
    }
}
