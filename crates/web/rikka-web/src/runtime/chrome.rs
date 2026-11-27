use serde::Deserialize;

use crate::ids::{TabId, WindowId};
use crate::runtime::joint::Step;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MutedInfo {
    #[serde(default)]
    pub(crate) muted: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tab {
    pub(crate) id: Option<TabId>,
    pub(crate) window_id: WindowId,
    pub(crate) opener_tab_id: Option<TabId>,
    pub(crate) url: Option<String>,
    pub(crate) pending_url: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) fav_icon_url: Option<String>,
    #[serde(default)]
    pub(crate) active: bool,
    #[serde(default)]
    pub(crate) audible: bool,
    pub(crate) muted_info: Option<MutedInfo>,
    #[serde(default)]
    pub(crate) discarded: bool,
}

impl Tab {
    pub(crate) fn address(&self) -> String {
        self.pending_url
            .clone()
            .or_else(|| self.url.clone())
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChromeWindow {
    pub(crate) id: Option<WindowId>,
    #[serde(rename = "type")]
    pub(crate) kind: Option<String>,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    #[serde(default)]
    pub(crate) tabs: Vec<Tab>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Navigation {
    pub(crate) tab_id: TabId,
    pub(crate) frame_id: i64,
    pub(crate) url: String,
    pub(crate) transition_type: Option<String>,
    #[serde(default)]
    pub(crate) transition_qualifiers: Vec<String>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActiveInfo {
    pub(crate) tab_id: TabId,
    pub(crate) window_id: WindowId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZoomChange {
    pub(crate) tab_id: TabId,
    pub(crate) new_zoom_factor: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Origin {
    pub(crate) tab: Option<TabId>,
    #[serde(default)]
    pub(crate) frame: i64,
    pub(crate) url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageReport {
    pub(crate) step: Option<Step>,
    pub(crate) url: String,
    pub(crate) length: usize,
    #[serde(default)]
    pub(crate) back: usize,
    #[serde(default)]
    pub(crate) forward: usize,
    #[serde(default)]
    pub(crate) fullscreen: bool,
}
