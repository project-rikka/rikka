#[derive(Clone, Debug, PartialEq)]
pub struct PageState {
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub favicon: Option<String>,
    pub audible: bool,
    pub muted: bool,
    pub fullscreen: bool,
    pub zoom: f64,
    pub discarded: bool,
    pub hovered_link: Option<String>,
}

impl Default for PageState {
    fn default() -> PageState {
        PageState {
            url: String::new(),
            title: String::new(),
            loading: false,
            can_go_back: false,
            can_go_forward: false,
            favicon: None,
            audible: false,
            muted: false,
            fullscreen: false,
            zoom: 1.0,
            discarded: false,
            hovered_link: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransitionKind {
    Link,
    Typed,
    Bookmark,
    Generated,
    StartPage,
    FormSubmit,
    Reload,
    Keyword,
    Other,
}

impl TransitionKind {
    pub(crate) fn parse(kind: &str) -> TransitionKind {
        match kind {
            "link" => TransitionKind::Link,
            "typed" => TransitionKind::Typed,
            "auto_bookmark" => TransitionKind::Bookmark,
            "generated" => TransitionKind::Generated,
            "start_page" | "auto_toplevel" => TransitionKind::StartPage,
            "form_submit" => TransitionKind::FormSubmit,
            "reload" => TransitionKind::Reload,
            "keyword" | "keyword_generated" => TransitionKind::Keyword,
            _ => TransitionKind::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Transition {
    pub kind: TransitionKind,
    pub redirect: bool,
    pub back_forward: bool,
    pub from_address_bar: bool,
}

impl Transition {
    pub(crate) fn parse(kind: &str, qualifiers: &[String]) -> Transition {
        let has = |qualifier: &str| qualifiers.iter().any(|known| known == qualifier);
        Transition {
            kind: TransitionKind::parse(kind),
            redirect: has("client_redirect") || has("server_redirect"),
            back_forward: has("forward_back"),
            from_address_bar: has("from_address_bar"),
        }
    }
}
