use rikka_web::OpenWhere;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opening {
    Here,
    BackgroundTab,
    ForegroundTab,
    Window,
}

pub(crate) fn opening(target: OpenWhere) -> Opening {
    match target {
        OpenWhere::NewTabBackground => Opening::BackgroundTab,
        OpenWhere::NewTabForeground => Opening::ForegroundTab,
        OpenWhere::NewWindow => Opening::Window,
        OpenWhere::DefaultWindow
        | OpenWhere::CurrentWindow
        | OpenWhere::NewTab
        | OpenWhere::NewTabAfterCurrent
        | OpenWhere::PrintBrowser => Opening::Here,
    }
}
