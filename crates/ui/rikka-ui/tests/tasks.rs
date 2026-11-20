use rikka_theme::Theme;
use rikka_types::Size;
use rikka_ui::*;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Recorded {
    Started,
    Reload,
    Loaded(u32),
}

#[derive(Default, State)]
struct Screen {
    log: Vec<Recorded>,
}

impl Screen {
    fn start(&mut self) {
        self.log.push(Recorded::Started);
        spawn_blocking(|| 7_u32, Screen::loaded);
    }

    fn reload(&mut self) {
        self.log.push(Recorded::Reload);
        spawn_blocking(|| 9_u32, Screen::loaded);
    }

    fn loaded(&mut self, value: u32) {
        self.log.push(Recorded::Loaded(value));
    }
}

fn counted(screen: &Screen) -> impl View {
    text(format!("{} messages", screen.log.len()))
}

fn screen() -> TypedUi<Screen> {
    let mut ui = TypedUi::new(Screen::default(), counted, Theme::dark());
    ui.set_viewport(Size::new(200.0, 100.0), 1.0);
    ui.start(Screen::start);
    ui
}

fn run(job: Job) -> JobResult {
    let Job::Start {
        id,
        work: Work::Blocking(work),
        ..
    } = job
    else {
        panic!("unexpected async work");
    };
    JobResult {
        job: id,
        output: Some(work()),
    }
}

#[test]
fn jobs_are_delivered_once() {
    let mut ui = screen();

    assert_eq!(ui.take_jobs().len(), 1);
    assert!(ui.take_jobs().is_empty(), "job delivered twice");
}

#[test]
fn update_work_is_queued() {
    let mut ui = screen();
    ui.take_jobs();

    ui.update(Screen::reload);
    let job = ui.take_jobs().pop().expect("update requested no work");
    ui.complete_job(run(job));

    assert_eq!(
        ui.state().log,
        [Recorded::Started, Recorded::Reload, Recorded::Loaded(9)]
    );
}
