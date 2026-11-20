use rikka_app::prelude::*;
use rikka_omnibox::UrlDisplay;
use rikka_ui::Handler;
use rikka_widgets::row;
use url::Url;

use crate::corners::Corners;
use crate::internal::route::Presented;
use crate::omnibox::{self, Omnibox};
use crate::shell::Shell;
use crate::translate::Tool;
use crate::widgets::context_area;
use browser_model::TabId;

pub(crate) const FIELD: &str = "rikka-browser-address";
const TRANSLATE: &str = "rikka-browser-address-translate";
const HEIGHT: f32 = 28.0;
const GLYPH: f32 = 15.0;
const TOOL: f32 = 24.0;
const RADIUS: f32 = Corner::Control.radius();
const FIELD_SIZE: ControlSize = ControlSize::Small;

#[derive(State)]
pub(crate) struct Address {
    #[untracked]
    field: TextDocument,
    editing: Tracked<bool>,
    translate_popover: Tracked<Option<TabId>>,
    #[state]
    pub(crate) omnibox: Omnibox,
}

impl Address {
    pub(crate) fn new() -> Address {
        Address {
            field: TextDocument::new(""),
            editing: Tracked::new(false),
            translate_popover: Tracked::new(None),
            omnibox: Omnibox::new(),
        }
    }

    pub(crate) fn begin(&mut self, current: String) {
        let (shown, url) = editable(&current);
        self.field.reset(&shown);
        self.field.select_all();
        self.omnibox.begin(shown, url);
        self.editing.set(true);
    }

    pub(crate) fn end(&mut self) {
        self.omnibox.end();
        if *self.editing {
            self.editing.set(false);
        }
    }

    pub(crate) fn editing(&self) -> bool {
        *self.editing
    }

    pub(crate) fn field(&self) -> &TextDocument {
        &self.field
    }

    pub(crate) fn translate_popover(&self) -> Option<TabId> {
        *self.translate_popover
    }

    pub(crate) fn show_translate_popover(&mut self, shown: Option<TabId>) {
        if *self.translate_popover != shown {
            self.translate_popover.set(shown);
        }
    }
}

impl std::fmt::Debug for Address {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Address")
            .field("editing", &*self.editing)
            .finish_non_exhaustive()
    }
}

fn editable(current: &str) -> (String, Option<Url>) {
    match Url::parse(current) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => {
            (UrlDisplay::full(&url).into_text(), Some(url))
        }
        Ok(url) => (current.to_owned(), Some(url)),
        Err(_) => (current.to_owned(), None),
    }
}

pub(crate) fn view(shell: &Shell) -> Element {
    let model = shell.model();
    let tab_entry = model.active(shell.id());
    let internal = tab_entry.and_then(|tab_entry| (*tab_entry.internal).clone());
    let (symbol, tint) = match (
        &internal,
        tab_entry.map(|tab_entry| &*tab_entry.meta.security),
    ) {
        (Some(internal), _) => (internal.symbol(), palette().text_muted),
        (None, Some(browser_model::Security::Unknown) | None) => {
            (MaterialSymbol::SEARCH, palette().text_muted)
        }
        (None, Some(security)) => crate::site_info::security_glyph(security),
    };
    let zoom = tab_entry.map_or(1.0, |tab_entry| *tab_entry.meta.zoom);
    let capture = tab_entry
        .map(|tab_entry| *tab_entry.meta.capture)
        .unwrap_or_default();
    let address = tab_entry
        .map(|tab_entry| tab_entry.address())
        .unwrap_or_default();
    let web = tab_entry.is_some_and(|tab_entry| tab_entry.internal.is_none())
        && browser_pages::web_site(&address).is_some();
    let tab = tab_entry.map(|tab_entry| tab_entry.id);
    let readable = tab_entry.is_some_and(|tab_entry| shell.reader.offers(tab_entry.id));
    let reading = tab_entry.is_some_and(|tab_entry| shell.reader.reads(tab_entry.id));
    drop(model);
    let corners = Corners::for_shell(shell);
    let editing = shell.address().editing();
    let chip = editing
        .then(|| shell.address().omnibox.chip().cloned())
        .flatten();
    let body = match editing {
        true => field(shell, chip.as_ref()),
        false => shown(&address),
    };
    let reader = (readable || reading).then(|| {
        tool(
            corners,
            MaterialSymbol::CHROME_READER_MODE,
            match reading {
                true => palette().accent,
                false => palette().text_muted,
            },
        )
        .on_press(|shell: &mut Shell| shell.toggle_reader())
    });
    let translate = tab
        .filter(|_| web && !editing)
        .and_then(|tab| crate::translate::tool(shell, tab, &address))
        .map(|look| {
            let content = match look {
                Tool::Busy => spinner()
                    .extent(Length::px(GLYPH))
                    .color(palette().accent)
                    .any(),
                Tool::Offered | Tool::Translated | Tool::Failed => icon(MaterialSymbol::TRANSLATE)
                    .extent(Length::px(GLYPH + 1.0))
                    .color(match look {
                        Tool::Translated => palette().accent,
                        Tool::Failed => palette().danger,
                        Tool::Offered | Tool::Busy => palette().text_muted,
                    })
                    .any(),
            };
            tool_slot(corners, content)
                .on_press(|shell: &mut Shell| shell.open_translate_popover())
                .build()
                .keyed(TRANSLATE)
        });
    let badge = ((zoom - 1.0).abs() > 0.001).then(|| {
        button(text(format!("{:.0}%", zoom * 100.0)).caption())
            .ghost()
            .small()
            .focusless()
            .on_press(|shell: &mut Shell| shell.zoom(0))
    });
    let chip_view = chip.map(|chip| {
        container(
            text(tr!("browser.omnibox.chip", engine = &chip.name))
                .caption()
                .medium(),
        )
        .padding(Inset::xy(Space::SNUG, Space::HAIR))
        .background(palette().accent.with_alpha(0.16))
        .radius(corners.pill(HEIGHT, RADIUS))
    });
    let bar = container(
        row! {
            crate::site_info::badge(symbol, tint, web);
            chip_view;
            body;
            crate::site_info::capturing(capture);
            reader;
            translate;
            badge;
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill(),
    )
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .background(palette().text.with_alpha(0.06))
    .radius(corners.pill(HEIGHT, RADIUS));
    let bar = match editing {
        true => bar,
        false => bar
            .on_press(|shell: &mut Shell| shell.focus_address())
            .focusless(),
    }
    .height(HEIGHT)
    .fill_width()
    .build()
    .keyed(FIELD);
    let open = Handler::new(|shell: &mut Shell, _: Point| shell.summon_address_menu());
    let translation = crate::translate::popover(shell, TRANSLATE);
    let actions = tab
        .filter(|_| !editing)
        .and_then(|tab| crate::extensions::actions(shell, tab));
    row_of([
        context_area(bar, open)
            .height(HEIGHT)
            .grow(1.0)
            .min_width(0.0)
            .any(),
        actions.unwrap_or_else(Element::empty),
        component(|omnibox: &Omnibox| omnibox, omnibox::dropdown).any(),
        translation.unwrap_or_else(Element::empty),
    ])
    .align_items(Align::Center)
    .height(HEIGHT)
    .fill_width()
    .any()
}

fn field(shell: &Shell, chip: Option<&omnibox::Chip>) -> Element {
    let placeholder = match chip {
        Some(chip) => tr!("browser.omnibox.search-engine", engine = &chip.name),
        None => tr!("browser.address.placeholder"),
    };
    text_field(shell.address().field())
        .placeholder(placeholder)
        .control_size(FIELD_SIZE)
        .font_size(env().font_size * TextRole::Label.scale())
        .plain(true)
        .claims_focus(true)
        .leaves_arrows(false)
        .on_change(|shell: &mut Shell| shell.address_edited())
        .on_submit(|shell: &mut Shell| shell.submit_address())
        .on_blur(|shell: &mut Shell| shell.leave_address())
        .on_key(|shell: &mut Shell, key: Key, modifiers: Modifiers| {
            shell.address_key(key, modifiers)
        })
        .on_command(|shell: &mut Shell, command: Command| shell.address_command(command))
        .context_items(Shell::address_menu)
        .height(HEIGHT)
        .grow(1.0)
        .min_width(0.0)
        .any()
}

fn tool(corners: Corners, symbol: MaterialSymbol, tint: Color) -> Container {
    tool_slot(
        corners,
        icon(symbol).extent(Length::px(GLYPH + 1.0)).color(tint),
    )
}

fn tool_slot(corners: Corners, content: impl View) -> Container {
    container(content)
        .center()
        .radius(corners.pill(TOOL, RADIUS))
        .focusless()
        .size(Length::px(TOOL), Length::px(TOOL))
}

fn shown(current: &str) -> Element {
    let words = match Url::parse(current) {
        _ if current.is_empty() => text(tr!("browser.address.placeholder"))
            .label()
            .muted()
            .no_wrap()
            .any(),
        Ok(url) if matches!(url.scheme(), "http" | "https") => {
            emphasised(&UrlDisplay::steady(&url))
        }
        _ => text(current.to_owned()).label().no_wrap().any(),
    };
    container(words)
        .padding(Inset::edges(0.0, 0.0, 0.0, FIELD_SIZE.text_inset()))
        .align_y(Align::Center)
        .grow(1.0)
        .min_width(0.0)
        .fill_height()
        .any()
}

fn emphasised(display: &UrlDisplay) -> Element {
    let mut spans = vec![TextSpan::new(0..display.text().len()).color(palette().text_muted)];
    if let Some(host) = display.host() {
        spans.push(TextSpan::new(host).color(palette().text));
    }
    rich_text(display.text().to_owned())
        .spans(spans)
        .label()
        .no_wrap()
        .shrink(1.0)
        .min_width(0.0)
        .any()
}
