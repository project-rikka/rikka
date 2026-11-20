use rikka_types::{Point, Rect};
use serde::{Deserialize, Serialize};

use crate::area::Area;
use crate::widget::{Widget, WidgetId};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    #[serde(default, rename = "widget")]
    pub widgets: Vec<Widget>,
}

impl Layout {
    #[must_use]
    pub fn get(&self, id: WidgetId) -> Option<&Widget> {
        self.widgets.iter().find(|widget| widget.id == id)
    }

    pub fn get_mut(&mut self, id: WidgetId) -> Option<&mut Widget> {
        self.widgets.iter_mut().find(|widget| widget.id == id)
    }

    pub fn remove(&mut self, id: WidgetId) -> Option<Widget> {
        let at = self.widgets.iter().position(|widget| widget.id == id)?;
        Some(self.widgets.remove(at))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }

    #[must_use]
    pub fn rect_of(&self, id: WidgetId, area: &Area) -> Option<Rect> {
        self.get(id)
            .map(|widget| area.place(widget.size, widget.spot))
    }

    #[must_use]
    pub fn rects(&self, area: &Area) -> Vec<(WidgetId, Rect)> {
        self.widgets
            .iter()
            .map(|widget| (widget.id, area.place(widget.size, widget.spot)))
            .collect()
    }

    #[must_use]
    pub fn widget_at(&self, area: &Area, at: Point) -> Option<(WidgetId, Rect)> {
        self.rects(area)
            .into_iter()
            .rev()
            .find(|(_, rect)| rect.contains(at))
    }

    #[must_use]
    pub fn rects_except(&self, id: WidgetId, area: &Area) -> Vec<Rect> {
        self.widgets
            .iter()
            .filter(|widget| widget.id != id)
            .map(|widget| area.place(widget.size, widget.spot))
            .collect()
    }
}
