use std::cell::Cell;
use std::rc::Rc;

use rikka_ui::{
    Anchor, Axis, Color, Command, Handler, Inset, Key, MaterialSymbol, Modifiers, Space, Style,
    Text, Transition,
};
use rikka_widgets::{
    Banner, Button, Checkbox, Collection, Combo, Container, Flex, Gauge, Icon, Image, Lazy, List,
    ListItem, MenuBar, NavItem, NavList, NumberField, Progress, Radio, Segmented, Sheet, Slider,
    Spinner, StatusPage, Switch, Tab, Tabs, TextEditor, TextField,
};

use crate::view::drop::Drops;
use crate::view::menu::MenuEntry;

type Edit<T> = Box<dyn FnOnce(T) -> T>;

pub(crate) enum Built<T> {
    Ready(Option<T>),
    Deferred(Vec<Edit<T>>),
}

impl<T: 'static> Built<T> {
    pub(crate) fn ready(value: T) -> Self {
        Self::Ready(Some(value))
    }

    pub(crate) fn deferred() -> Self {
        Self::Deferred(Vec::new())
    }

    pub(crate) fn edit(&mut self, edit: impl FnOnce(T) -> T + 'static) {
        match self {
            Self::Ready(slot) => {
                let value = slot.take().expect("a ready builder holds its value");
                *slot = Some(edit(value));
            }
            Self::Deferred(edits) => edits.push(Box::new(edit)),
        }
    }

    pub(crate) fn finish(self, make: impl FnOnce() -> T) -> T {
        match self {
            Self::Ready(slot) => slot.expect("a ready builder holds its value"),
            Self::Deferred(edits) => edits.into_iter().fold(make(), |value, edit| edit(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stack {
    pub(crate) gap: f32,
    pub(crate) padding: Inset,
}

impl Stack {
    pub(crate) fn new(gap: Space, padding: f32) -> Self {
        Self {
            gap: gap.px(),
            padding: Inset::all(padding),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Root {
    Window,
    Fragment,
    ListItem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Parent,
    Last,
}

pub(crate) struct Segment {
    pub(crate) label: String,
    pub(crate) icon: Option<MaterialSymbol>,
}

pub(crate) enum Kind {
    Root(Root),
    Flex(Built<Flex>),
    Box {
        container: Built<Container>,
        stack: Stack,
    },
    Scroll {
        follow: bool,
    },
    Page,
    Section {
        heading: String,
        stack: Stack,
    },
    Card {
        stack: Stack,
    },
    Field {
        label: String,
    },
    Button(Built<Button>),
    ButtonFrame {
        button: Built<Button>,
        stack: Stack,
    },
    Split {
        axis: Axis,
        ratio: f32,
        resize: Option<Handler<(f32,)>>,
    },
    PageStack {
        page: usize,
    },
    Sheet {
        stack: Stack,
        dismiss: Option<Handler>,
    },
    Popover {
        stack: Stack,
        dismiss: Option<Handler>,
    },
    StatusPage(Built<StatusPage>),
    ContextMenu {
        target: Target,
    },
    PopupMenu {
        dismiss: Option<Handler>,
    },
    MenuBar(Built<MenuBar>),
    Menu {
        title: String,
    },
    Submenu(MenuEntry),
    Segmented(Built<Segmented>),
    Combo(Built<Combo>),
    Tabs(Built<Tabs>),
    NavList(Built<NavList>),
    List(Built<List>),
    Segment(Segment),
    Option(String),
    Tab(Built<Tab>),
    NavItem(Built<NavItem>),
    ListItem(Built<ListItem>),
    MenuEntry(MenuEntry),
    Text(Built<Text>),
    Icon(Built<Icon>),
    Picture(Built<Image>),
    Spacer,
    Divider {
        vertical: bool,
    },
    Checkbox(Built<Checkbox>),
    Toggle(Built<Switch>),
    Radio(Built<Radio>),
    Slider(Built<Slider>),
    NumberField(Built<NumberField>),
    Progress(Built<Progress>),
    Spinner(Built<Spinner>),
    Gauge(Built<Gauge>),
    TextField(Built<TextField>),
    TextEditor(Built<TextEditor>),
    Banner(Box<Built<Banner>>),
    Unsaved(Box<Sheet>),
    LazyList {
        list: Built<List>,
        selected: Rc<Cell<Option<usize>>>,
    },
    Collection(Built<Collection>),
    Memo(Built<Lazy>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Holds {
    Elements,
    Segments,
    Options,
    Tabs,
    NavItems,
    ListItems,
    MenuEntries,
    Menus,
    Nothing,
}

impl Kind {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Root(_) => "the view",
            Self::Flex(_) => "a row, column or grid",
            Self::Box { .. } => "a box",
            Self::Scroll { .. } => "a scroll area",
            Self::Page => "a page",
            Self::Section { .. } => "a section",
            Self::Card { .. } => "a card",
            Self::Field { .. } => "a field",
            Self::Button(_) | Self::ButtonFrame { .. } => "a button",
            Self::Split { .. } => "a split",
            Self::PageStack { .. } => "a page stack",
            Self::Sheet { .. } => "a sheet",
            Self::Popover { .. } => "a popover",
            Self::StatusPage(_) => "a status page",
            Self::ContextMenu { .. } => "a context menu",
            Self::PopupMenu { .. } => "a popup menu",
            Self::MenuBar(_) => "a menu bar",
            Self::Menu { .. } => "a menu",
            Self::Submenu(_) => "a submenu",
            Self::Segmented(_) => "a segmented control",
            Self::Combo(_) => "a combo",
            Self::Tabs(_) => "tabs",
            Self::NavList(_) => "a nav list",
            Self::List(_) => "a list",
            Self::Segment(_) => "a segment",
            Self::Option(_) => "an option",
            Self::Tab(_) => "a tab",
            Self::NavItem(_) => "a nav item",
            Self::ListItem(_) => "a list item",
            Self::MenuEntry(_) => "a menu item",
            Self::Text(_) => "text",
            Self::Icon(_) => "an icon",
            Self::Picture(_) => "a picture",
            Self::Spacer => "a spacer",
            Self::Divider { .. } => "a divider",
            Self::Checkbox(_) => "a checkbox",
            Self::Toggle(_) => "a switch",
            Self::Radio(_) => "a radio button",
            Self::Slider(_) => "a slider",
            Self::NumberField(_) => "a number field",
            Self::Progress(_) => "a progress bar",
            Self::Spinner(_) => "a spinner",
            Self::Gauge(_) => "a gauge",
            Self::TextField(_) => "a text field",
            Self::TextEditor(_) => "a text editor",
            Self::Banner(_) => "a banner",
            Self::Unsaved(_) => "an unsaved-changes prompt",
            Self::LazyList { .. } => "a lazy list",
            Self::Collection(_) => "a lazy collection",
            Self::Memo(_) => "a memo",
        }
    }

    pub(crate) fn holds(&self) -> Holds {
        match self {
            Self::Root(Root::ListItem) => Holds::ListItems,
            Self::Root(_)
            | Self::Flex(_)
            | Self::Box { .. }
            | Self::Scroll { .. }
            | Self::Page
            | Self::Section { .. }
            | Self::Card { .. }
            | Self::Field { .. }
            | Self::Split { .. }
            | Self::PageStack { .. }
            | Self::Sheet { .. }
            | Self::Popover { .. }
            | Self::StatusPage(_)
            | Self::ButtonFrame { .. } => Holds::Elements,
            Self::Segmented(_) => Holds::Segments,
            Self::Combo(_) => Holds::Options,
            Self::Tabs(_) => Holds::Tabs,
            Self::NavList(_) => Holds::NavItems,
            Self::List(_) => Holds::ListItems,
            Self::ContextMenu { .. }
            | Self::PopupMenu { .. }
            | Self::Menu { .. }
            | Self::Submenu(_) => Holds::MenuEntries,
            Self::MenuBar(_) => Holds::Menus,
            _ => Holds::Nothing,
        }
    }

    pub(crate) fn belongs_in(&self) -> Holds {
        match self {
            Self::Segment(_) => Holds::Segments,
            Self::Option(_) => Holds::Options,
            Self::Tab(_) => Holds::Tabs,
            Self::NavItem(_) => Holds::NavItems,
            Self::ListItem(_) => Holds::ListItems,
            Self::MenuEntry(_) | Self::Submenu(_) => Holds::MenuEntries,
            Self::Menu { .. } => Holds::Menus,
            Self::ContextMenu { .. } => Holds::Nothing,
            _ => Holds::Elements,
        }
    }

    pub(crate) fn is_element(&self) -> bool {
        self.belongs_in() == Holds::Elements && !matches!(self, Self::Root(_))
    }
}

#[derive(Default)]
pub(crate) struct Common {
    pub(crate) style: Style,
    pub(crate) key: Option<u64>,
    pub(crate) tooltip: Option<String>,
    pub(crate) opacity: Option<f32>,
    pub(crate) scale: Option<f32>,
    pub(crate) background: Option<Color>,
    pub(crate) transition: Option<Transition>,
    pub(crate) anchor: Option<Anchor>,
    pub(crate) shortcuts: Vec<(Key, Modifiers, Handler)>,
    pub(crate) commands: Vec<(Command, Handler)>,
    pub(crate) menu: Option<Rc<Vec<MenuEntry>>>,
    pub(crate) drops: Drops,
}

pub(crate) struct Node {
    pub(crate) kind: Kind,
    pub(crate) common: Common,
    pub(crate) children: Vec<Node>,
}

impl Node {
    pub(crate) fn new(kind: Kind) -> Self {
        Self {
            kind,
            common: Common::default(),
            children: Vec::new(),
        }
    }

    pub(crate) fn refuse(&self, what: &str) -> ! {
        crate::abi::violation(format!("{} has no {what}", self.kind.name()))
    }

    pub(crate) fn refuse_reaction(&self, what: &str) -> ! {
        crate::abi::violation(format!("{} does not react to {what}", self.kind.name()))
    }
}
