use std::process::ExitCode;
use std::rc::Rc;

use iced::widget::operation::scroll_to;
use iced::widget::scrollable::AbsoluteOffset;
use iced::{Element, Size, Subscription, Task, Theme};
use ui_iced_views::{LIST_ID, Message};
use ui_scene::{Bounce, Grid, LIST_ROWS, Probe, Row, Scenario, Table, WINDOW_HEIGHT, WINDOW_WIDTH};

enum Content {
    Grid(Grid),
    List { rows: Rc<[Row]>, bounce: Bounce },
}

struct Bench {
    content: Content,
    scenario: Scenario,
    probe: Probe,
}

impl Bench {
    fn new(scenario: Scenario) -> Bench {
        let content = match scenario.grid() {
            Some(grid) => Content::Grid(grid),
            None => Content::List {
                rows: Table::with_rows(LIST_ROWS).rows.into(),
                bounce: Bounce::new(LIST_ROWS),
            },
        };
        Bench {
            content,
            scenario,
            probe: Probe::from_env(),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let Message::Frame = message else {
            return Task::none();
        };
        self.probe.frame();
        match &mut self.content {
            Content::Grid(grid) => {
                grid.advance();
                Task::none()
            }
            Content::List { bounce, .. } => scroll_to(
                LIST_ID,
                AbsoluteOffset {
                    x: 0.0,
                    y: bounce.advance(),
                },
            ),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match &self.content {
            Content::Grid(grid) => ui_iced_views::grid(grid),
            Content::List { rows, .. } => ui_iced_views::list(Rc::clone(rows)),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match self.scenario.animates() {
            true => iced::window::frames().map(|_| Message::Frame),
            false => Subscription::none(),
        }
    }
}

fn main() -> ExitCode {
    let scenario = match Scenario::from_args() {
        Ok(scenario) => scenario,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let ran = iced::application(move || Bench::new(scenario), Bench::update, Bench::view)
        .subscription(Bench::subscription)
        .theme(|_: &Bench| Theme::Dark)
        .window_size(Size::new(WINDOW_WIDTH, WINDOW_HEIGHT))
        .run();
    match ran {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to run iced: {error}");
            ExitCode::FAILURE
        }
    }
}
