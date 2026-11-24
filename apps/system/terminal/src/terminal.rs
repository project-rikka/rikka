use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use rikka_app::prelude::*;
use rikka_terminal::{Config, Report, screen};
use rikka_ui::search::Action;
use rikka_widgets::{
    ControlSize, DropTarget, Side, Tabs, column, find_bar, find_mark, pane, split, tabs,
};

use crate::command::Command;
use crate::group::Group;
use crate::keys;
use crate::opening::{Opening, Program};
use crate::panes::Panes;
use crate::tab::Tab;

const SIZE_SAVE_DELAY: Duration = Duration::from_millis(400);

const SIZE_SAVE_KEY: &str = "rikka.terminal.remember-size";

const TAB_TRANSFER: &str = "rikka.terminal.tab";

const BORDER_WIDTH: f32 = 1.0;

const FIND_MARK: &str = "rikka.terminal.find";

const RESIZE_STEP: f32 = 0.05;

#[derive(State)]
pub(crate) struct Terminal {
    #[untracked]
    panes: Panes,
    shape: Revision,
    active: Tracked<u64>,
    #[untracked]
    sender: Sender<Terminal>,
    config: Tracked<Config>,
    error: Tracked<Option<String>>,
    #[untracked]
    next_tab: u64,
    #[untracked]
    next_node: u64,
    #[untracked]
    cwd: Option<PathBuf>,
}

impl Terminal {
    fn new(sender: Sender<Terminal>, opening: Opening) -> Self {
        let mut terminal = Self::empty(sender, opening.dir);
        let program = match opening.program {
            Some(Err(unreadable)) => {
                terminal.error.set(Some(unreadable.to_string()));
                return terminal;
            }
            Some(Ok(program)) => Some(program),
            None => None,
        };
        let group = *terminal.active;
        if let Some(tab) = terminal.spawn_tab(program.as_ref()) {
            terminal.insert_tab(group, usize::MAX, tab);
        }
        terminal
    }

    fn with_tab(sender: Sender<Terminal>, tab: Tab, cwd: Option<PathBuf>) -> Self {
        let mut terminal = Self::empty(sender, cwd);
        terminal.insert_moved(*terminal.active, 0, tab);
        terminal
    }

    fn empty(sender: Sender<Terminal>, cwd: Option<PathBuf>) -> Self {
        Self {
            panes: Panes::Leaf(Group::new(0)),
            shape: Revision::new(),
            active: Tracked::new(0),
            sender,
            config: Tracked::new(Config::load()),
            error: Tracked::new(None),
            next_tab: 0,
            next_node: 1,
            cwd,
        }
    }

    fn next_node(&mut self) -> u64 {
        let id = self.next_node;
        self.next_node += 1;
        id
    }

    fn next_tab(&mut self) -> u64 {
        let id = self.next_tab;
        self.next_tab += 1;
        id
    }

    fn open_tab(&mut self, group: u64) {
        if let Some(tab) = self.spawn_tab(None) {
            self.insert_tab(group, usize::MAX, tab);
        }
    }

    fn spawn_tab(&mut self, program: Option<&Program>) -> Option<Tab> {
        let id = self.next_tab();
        let at = self.working_directory();
        match Tab::open(id, &self.config, &self.sender, at.as_deref(), program) {
            Ok(tab) => Some(tab),
            Err(error) => {
                self.error.set(Some(error.to_string()));
                None
            }
        }
    }

    fn working_directory(&self) -> Option<PathBuf> {
        self.active_tab()
            .and_then(Tab::working_directory)
            .or_else(|| self.cwd.clone())
    }

    fn active_program(&self) -> Option<Program> {
        self.active_tab().and_then(Tab::program).cloned()
    }

    fn claim(&mut self, tab: Tab) -> Tab {
        let id = self.next_tab();
        tab.reassigned(id, &self.sender)
    }

    fn insert_moved(&mut self, group: u64, at: usize, tab: Tab) {
        let tab = self.claim(tab);
        self.insert_tab(group, at, tab);
    }

    fn insert_tab(&mut self, group: u64, at: usize, tab: Tab) {
        let Some(pane) = self.panes.group_mut(group) else {
            return;
        };
        pane.insert(at, tab);
        self.active.set(group);
        self.error.set(None);
    }

    fn take_tab(&mut self, group: u64, index: usize) -> Option<Tab> {
        self.panes.group_mut(group)?.remove(index)
    }

    fn tear_off(&self, tab: Tab) {
        let cwd = tab.working_directory().or_else(|| self.cwd.clone());
        open_window(window_with_tab(tab, cwd));
    }

    fn activate(&mut self, group: u64) {
        if self.panes.group(group).is_some() {
            self.active.set(group);
        }
    }

    fn select(&mut self, group: u64, index: usize) {
        if let Some(pane) = self.panes.group_mut(group) {
            pane.select(index);
            self.active.set(group);
        }
    }

    fn close_if_empty(&mut self, group: u64) {
        if self.panes.group(group).is_some_and(Group::is_empty) {
            self.close_pane(group);
        }
    }

    fn close_tab(&mut self, group: u64, index: usize) {
        let Some(pane) = self.panes.group_mut(group) else {
            return;
        };
        if pane.remove(index).is_none() || !pane.is_empty() {
            return;
        }
        self.close_pane(group);
    }

    fn close_pane(&mut self, group: u64) {
        let successor = self.panes.successor_of(group);
        self.shape.changed();
        if !self.panes.remove_group(group) {
            request_window(WindowRequest::Close);
            return;
        }
        if *self.active == group {
            let successor = successor.unwrap_or_else(|| self.panes.first());
            self.active.set(successor);
        }
    }

    fn split_pane(&mut self, target: u64, side: Side, tab: Tab) {
        let group = self.next_node();
        let split = self.next_node();
        self.panes
            .split(target, side, split, Group::with_tab(group, tab));
        self.shape.changed();
        self.active.set(group);
        self.error.set(None);
    }

    fn split_open(&mut self, side: Side) {
        let program = self.active_program();
        if let Some(tab) = self.spawn_tab(program.as_ref()) {
            let target = *self.active;
            self.split_pane(target, side, tab);
        }
    }

    fn split_auto(&mut self) {
        let wide = self.active_tab().is_none_or(|tab| {
            let size = tab.session().pixel_size();
            size.width >= size.height
        });
        self.split_open(match wide {
            true => Side::Right,
            false => Side::Bottom,
        });
    }

    fn duplicate_tab(&mut self) {
        let active = *self.active;
        let Some(at) = self.panes.group(active).map(Group::active_index) else {
            return;
        };
        let program = self.active_program();
        if let Some(tab) = self.spawn_tab(program.as_ref()) {
            self.insert_tab(active, at + 1, tab);
        }
    }

    fn drop_tab(&mut self, group: u64, landing: DropTarget, tab: Tab) {
        let splittable = self.panes.group(group).is_some_and(|pane| !pane.is_empty());
        match landing {
            DropTarget::Split(side) if splittable => {
                let tab = self.claim(tab);
                self.split_pane(group, side, tab);
            }
            DropTarget::Split(_) | DropTarget::Into => self.insert_moved(group, usize::MAX, tab),
        }
    }

    fn active_tab(&self) -> Option<&Tab> {
        self.panes.group(*self.active)?.active_tab()
    }

    pub(crate) fn report(&mut self, tab: u64, report: Report) {
        match report {
            Report::Title(title) => self.retitle(tab, Some(title)),
            Report::ResetTitle => self.retitle(tab, None),
            Report::Exited => {
                if let Some((group, index)) = self.panes.locate_tab(tab) {
                    self.close_tab(group, index);
                }
            }
            Report::Ask(question) => {
                if let Some(open_tab) = self.panes.tab_mut(tab) {
                    open_tab.session().answer(question);
                }
                skip_view();
            }
        }
    }

    fn retitle(&mut self, tab: u64, title: Option<String>) {
        let retitled = self
            .panes
            .tab_mut(tab)
            .is_some_and(|open_tab| open_tab.set_title(title));
        if !retitled {
            skip_view();
        }
    }

    fn key_pressed(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let performed = keys::command(key, modifiers).is_some_and(|command| self.command(command));
        if !performed {
            skip_view();
        }
        performed
    }

    fn command(&mut self, command: Command) -> bool {
        let active = *self.active;
        match command {
            Command::NewWindow => open_window(window(Opening {
                dir: self.working_directory(),
                program: None,
            })),
            Command::CloseWindow => request_window(WindowRequest::AskToClose),
            Command::Fullscreen => request_window(WindowRequest::ToggleFullscreen),
            Command::NewTab => self.open_tab(active),
            Command::DuplicateTab => self.duplicate_tab(),
            Command::CloseTab => {
                if let Some(index) = self.panes.group(active).map(Group::active_index) {
                    self.close_tab(active, index);
                }
            }
            Command::NextTab | Command::PreviousTab => {
                if let Some(pane) = self.panes.group_mut(active) {
                    pane.step(command == Command::NextTab);
                }
            }
            Command::Tab(index) => {
                let Some(pane) = self.panes.group_mut(active) else {
                    return false;
                };
                if index >= pane.tabs().len() {
                    return false;
                }
                pane.select(index);
            }
            Command::LastTab => {
                if let Some(pane) = self.panes.group_mut(active) {
                    let last = pane.tabs().len().saturating_sub(1);
                    pane.select(last);
                }
            }
            Command::Split(side) => self.split_open(side),
            Command::SplitAuto => self.split_auto(),
            Command::FocusPane(side) => {
                let Some(next) = self.panes.neighbour(active, side) else {
                    return false;
                };
                self.active.set(next);
            }
            Command::ResizePane(side) => return self.panes.nudge(active, side, RESIZE_STEP),
            Command::Find => {
                if let Some(tab) = self.active_tab() {
                    tab.open_find();
                }
            }
            Command::Bigger => self.zoom(1),
            Command::Smaller => self.zoom(-1),
            Command::ResetSize => {
                self.config.reset_font();
                schedule_size_save();
            }
        }
        true
    }

    fn zoom(&mut self, steps: i32) {
        self.config.resize_font(steps);
        schedule_size_save();
    }
}

fn schedule_size_save() {
    after_keyed(SIZE_SAVE_KEY, SIZE_SAVE_DELAY, |terminal: &mut Terminal| {
        terminal.config.save_font_size();
        skip_view();
    });
}

fn find(panes: &mut Panes, tab: u64, action: Action) {
    if let Some(open_tab) = panes.tab_mut(tab) {
        open_tab.find(action);
    }
}

pub(crate) fn view(terminal: &Terminal) -> impl View {
    terminal.shape.read();
    let top_groups: Vec<u64> = terminal
        .panes
        .top_row()
        .iter()
        .map(|(group, _)| group.id())
        .collect();
    panes_view(&terminal.panes, &top_groups)
}

fn caption(terminal: &Terminal) -> Option<CaptionBar> {
    terminal.shape.read();
    Some(CaptionBar::columns(
        terminal.panes.top_row().into_iter().map(|(group, share)| {
            let id = group.id();
            let strip = component_with(|terminal: &Terminal| terminal, id, caption_strip);
            (share, strip.keyed(id))
        }),
    ))
}

fn caption_strip(terminal: &Terminal, group: &u64) -> Element {
    terminal.shape.read();
    match terminal.panes.group(*group) {
        Some(group) => strip(terminal, group).in_caption().build(),
        None => Element::empty(),
    }
}

fn strip(terminal: &Terminal, group: &Group) -> Tabs {
    let id = group.id();
    let untitled = tr!("terminal.tab.untitled");
    tabs(group.active_index())
        .items(group.tabs().iter().map(|tab| tab.strip(&untitled)))
        .on_select(move |terminal: &mut Terminal, index| terminal.select(id, index))
        .on_close(move |terminal: &mut Terminal, index| terminal.close_tab(id, index))
        .on_new(move |terminal: &mut Terminal| terminal.open_tab(id))
        .transfers(
            Transfer::group(TAB_TRANSFER)
                .take(move |terminal: &mut Terminal, index| terminal.take_tab(id, index))
                .insert(move |terminal: &mut Terminal, at, tab| terminal.insert_moved(id, at, tab))
                .tear_off(
                    |terminal: &mut Terminal, tab| terminal.tear_off(tab),
                    terminal.panes.is_split(),
                ),
        )
        .on_finished(move |terminal: &mut Terminal| terminal.close_if_empty(id))
}

fn panes_view(panes: &Panes, top_groups: &[u64]) -> Element {
    match panes {
        Panes::Leaf(group) => {
            let id = group.id();
            let in_caption = top_groups.contains(&id);
            component_with(|terminal: &Terminal| terminal, (id, in_caption), pane_part).keyed(id)
        }
        Panes::Split(node) => {
            let id = node.id();
            split(
                node.axis(),
                node.ratio(),
                panes_view(node.first(), top_groups),
                panes_view(node.second(), top_groups),
            )
            .on_resize(move |panes: &mut Panes, ratio| panes.resize(id, ratio))
            .build()
            .keyed(id)
        }
    }
}

fn pane_part(terminal: &Terminal, &(id, in_caption): &(u64, bool)) -> Element {
    terminal.shape.read();
    match terminal.panes.group(id) {
        Some(group) => pane_view(terminal, group, in_caption),
        None => Element::empty(),
    }
}

fn pane_view(terminal: &Terminal, group: &Group, in_caption: bool) -> Element {
    let id = group.id();
    let shown = match group.active_tab() {
        Some(tab) => grid_view(terminal, tab, id),
        None => error_view(terminal),
    };
    let body = page_stack(group.active_index(), shown)
        .named(group.active_tab().map(Tab::id))
        .grow(1.0)
        .fill_width()
        .build();
    let content = match terminal.panes.is_split() {
        true => framed(terminal, group, body, in_caption),
        false => body,
    };
    pane(content)
        .accepting(
            TAB_TRANSFER,
            move |terminal: &mut Terminal, landing, tab| terminal.drop_tab(id, landing, tab),
        )
        .build()
}

fn framed(terminal: &Terminal, group: &Group, body: Element, in_caption: bool) -> Element {
    let content = match in_caption {
        true => body,
        false => column! {
            strip(terminal, group).control_size(ControlSize::Small);
            body;
        }
        .fill()
        .build(),
    };
    container(content)
        .padding(BORDER_WIDTH)
        .border(BORDER_WIDTH, edge(terminal, group.id()))
        .grow(1.0)
        .build()
}

fn grid_view(terminal: &Terminal, tab: &Tab, group: u64) -> Element {
    let id = tab.id();
    column! {
        find_mark((FIND_MARK, id));
        screen(Rc::clone(tab.session()), terminal.config.font())
            .on_key(|terminal: &mut Terminal, key, modifiers| terminal.key_pressed(key, modifiers))
            .on_zoom(|terminal: &mut Terminal, steps| terminal.zoom(steps))
            .search(tab.search())
            .active(*terminal.active == group)
            .on_activate(move |terminal: &mut Terminal| terminal.activate(group))
            .grow(1.0)
            .build()
            .keyed(id);
        find_bar(tab.search())
            .anchored_to((FIND_MARK, id))
            .on_action(move |panes: &mut Panes, action| find(panes, id, action));
    }
    .grow(1.0)
    .fill_width()
    .build()
}

fn edge(terminal: &Terminal, group: u64) -> Color {
    let palette = palette();
    if *terminal.active == group {
        palette.accent
    } else {
        palette.outline
    }
}

fn error_view(terminal: &Terminal) -> Element {
    container(text(terminal.error.as_deref().unwrap_or_default()).color(TextColor::Danger))
        .center()
        .grow(1.0)
        .build()
}

pub(crate) fn window(opening: Opening) -> Window<Terminal> {
    configured(Window::new(
        move |sender: Sender<Terminal>| Terminal::new(sender, opening),
        view,
    ))
}

fn window_with_tab(tab: Tab, cwd: Option<PathBuf>) -> Window<Terminal> {
    configured(Window::new(
        move |sender: Sender<Terminal>| Terminal::with_tab(sender, tab, cwd),
        view,
    ))
}

fn configured(window: Window<Terminal>) -> Window<Terminal> {
    window
        .title(tr!("terminal.title"))
        .size(720.0, 460.0)
        .min_size(240.0, 160.0)
        .caption(caption)
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("panes", &self.panes)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}
