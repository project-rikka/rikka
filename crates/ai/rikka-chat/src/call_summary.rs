use rikka_thread::Call;
use rikka_tools::{Inspection, ToolAction};
use rikka_ui::MaterialSymbol;

#[derive(Debug)]
pub struct CallSummary<'a> {
    pub name: String,
    pub symbol: MaterialSymbol,
    pub gist: Option<String>,
    pub action: ToolAction<'a>,
    pub pictures: bool,
}

pub trait SummarizeCall: 'static {
    fn summarize<'a>(&self, call: &'a Call) -> CallSummary<'a>;
}

#[must_use]
pub fn symbol(action: &ToolAction<'_>) -> MaterialSymbol {
    match action {
        ToolAction::Command { .. } => MaterialSymbol::TERMINAL,
        ToolAction::Inspect { look, .. } => match look.inspection() {
            Inspection::Read => MaterialSymbol::DESCRIPTION,
            Inspection::Search => MaterialSymbol::MANAGE_SEARCH,
            Inspection::List => MaterialSymbol::FOLDER_OPEN,
        },
        ToolAction::View { .. } => MaterialSymbol::IMAGE,
        ToolAction::Write { .. } => MaterialSymbol::EDIT_DOCUMENT,
        ToolAction::Edit { .. } => MaterialSymbol::DIFFERENCE,
        ToolAction::Job { .. } => MaterialSymbol::PENDING_ACTIONS,
        ToolAction::Action { .. } | ToolAction::Operate(_) => MaterialSymbol::COMPUTER,
        ToolAction::Search { .. } => MaterialSymbol::TRAVEL_EXPLORE,
        ToolAction::Fetch { .. } => MaterialSymbol::PUBLIC,
        ToolAction::Ask { .. } => MaterialSymbol::LIVE_HELP,
        ToolAction::Tool { .. } | ToolAction::Unknown => MaterialSymbol::CONSTRUCTION,
    }
}
