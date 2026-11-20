use rikka_gecko_protocol::geckoview::content::ContextMenu as WireMenu;
use rikka_gecko_protocol::rikka::editing::{ContextMenu as RichMenu, Spelling as WireSpelling};
use rikka_types::Point;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MenuTarget {
    Page,
    Image,
    Video,
    Audio,
    Other(String),
}

impl MenuTarget {
    fn from_element_type(element_type: String) -> MenuTarget {
        match element_type.as_str() {
            "none" => MenuTarget::Page,
            "HTMLImageElement" => MenuTarget::Image,
            "HTMLVideoElement" => MenuTarget::Video,
            "HTMLAudioElement" => MenuTarget::Audio,
            _ => MenuTarget::Other(element_type),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spelling {
    pub misspelling: String,
    pub suggestions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContextMenu {
    pub at: Point,
    pub target: MenuTarget,
    pub base_uri: Option<String>,
    pub link_uri: Option<String>,
    pub link_text: Option<String>,
    pub source_uri: Option<String>,
    pub title: Option<String>,
    pub alt: Option<String>,
    pub text: Option<String>,
    pub selection: Option<String>,
    pub editable: bool,
    pub spelling: Option<Spelling>,
}

impl ContextMenu {
    pub(crate) fn from_wire(menu: WireMenu) -> ContextMenu {
        ContextMenu {
            at: Point::new(menu.screen_x as f32, menu.screen_y as f32),
            target: MenuTarget::from_element_type(menu.element_type),
            base_uri: menu.base_uri,
            link_uri: menu.uri,
            link_text: menu.link_text,
            source_uri: menu.element_src,
            title: menu.title,
            alt: menu.alt,
            text: menu.text_content,
            selection: None,
            editable: false,
            spelling: None,
        }
    }

    pub(crate) fn rich(menu: RichMenu) -> ContextMenu {
        ContextMenu {
            selection: menu.selection,
            editable: menu.editable,
            spelling: menu.spelling.map(
                |WireSpelling {
                     misspelling,
                     suggestions,
                 }| Spelling {
                    misspelling,
                    suggestions,
                },
            ),
            ..ContextMenu::from_wire(menu.menu)
        }
    }
}
