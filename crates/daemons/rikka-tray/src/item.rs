use crate::category::Category;
use crate::icon::Icon;
use crate::id::ItemId;
use crate::status::Status;
use crate::tooltip::Tooltip;

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: ItemId,
    pub name: String,
    pub title: String,
    pub category: Category,
    pub status: Status,
    pub icon: Option<Icon>,
    pub overlay: Option<Icon>,
    pub tooltip: Option<Tooltip>,
    pub is_menu: bool,
    pub menu: Option<String>,
    pub process: Option<String>,
}

impl Item {
    #[must_use]
    pub fn has_menu(&self) -> bool {
        self.menu.is_some()
    }

    #[must_use]
    pub fn accessible_name(&self) -> Option<&str> {
        [
            self.tooltip.as_ref().map_or("", |tooltip| &tooltip.title),
            &self.title,
        ]
        .into_iter()
        .map(str::trim)
        .find(|text| !text.is_empty())
    }

    #[must_use]
    pub fn label(&self) -> &str {
        self.accessible_name()
            .or_else(|| Some(self.name.trim()).filter(|name| !name.is_empty()))
            .unwrap_or(&self.id.service)
    }

    #[must_use]
    pub fn presses_open_menu(&self) -> bool {
        self.has_menu() && self.is_menu
    }
}
