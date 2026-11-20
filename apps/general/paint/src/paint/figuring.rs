use paint_document::{Figure, FigureChange, FigureId, LayerId, LayerKind, StackOrder};
use rikka_app::prelude::*;

use super::Paint;
use crate::figures::{Restyle, StyleSource, adopted, clipboard};
use crate::tab::Tab;
use crate::tool::Tool;

impl Paint {
    pub(crate) fn styled_tool(&self) -> Tool {
        let picked = self
            .active_tab()
            .filter(|_| self.tool.picks())
            .and_then(|tab| tab.picked_figures().last().map(|figure| figure.kind()));
        picked.map_or(self.tool, Tool::drawing_figure)
    }

    pub(crate) fn has_picks(&self) -> bool {
        self.unsaved_prompt.is_none()
            && self
                .active_tab()
                .is_some_and(|tab| tab.has_picks() && tab.figures_editable())
    }

    pub(crate) fn can_paste_figures(&self) -> bool {
        self.unsaved_prompt.is_none()
            && clipboard::holds()
            && self.active_tab().is_some_and(|tab| {
                tab.figures_editable()
                    && tab
                        .active_layer()
                        .is_some_and(|layer| layer.kind == LayerKind::Pixel && layer.visible)
            })
    }

    pub(crate) fn can_rasterize(&self) -> bool {
        self.unsaved_prompt.is_none()
            && self
                .active_tab()
                .is_some_and(|tab| tab.figures_editable() && tab.holds_figures())
    }

    pub(crate) fn can_pick_all(&self) -> bool {
        self.tool.picks() && self.can_rasterize()
    }

    pub(crate) fn pick_figures(&mut self, layer: LayerId, ids: Vec<FigureId>) {
        self.edit(|tab| tab.pick(layer, ids));
        self.adopt_picked();
    }

    pub(crate) fn unpick(&mut self) {
        self.edit(Tab::unpick);
    }

    pub(crate) fn pick_all_figures(&mut self) {
        self.edit(Tab::pick_all);
        self.adopt_picked();
    }

    pub(crate) fn preview_picked(&mut self, live: Vec<Figure>, copies: bool) {
        self.edit(|tab| tab.preview_picked(live, copies));
        skip_view();
    }

    pub(crate) fn commit_picked(&mut self, live: Vec<Figure>, by: FigureChange, copies: bool) {
        self.edit(|tab| tab.commit_picked(live, by, copies));
        self.adopt_picked();
    }

    pub(crate) fn drop_picked_preview(&mut self) {
        self.edit(Tab::drop_picked_preview);
    }

    pub(crate) fn nudge_figures(&mut self, by: (i32, i32)) {
        self.edit(|tab| tab.nudge_picked(by));
    }

    pub(crate) fn delete_figures(&mut self) {
        self.edit(Tab::remove_picked);
    }

    pub(crate) fn copy_figures(&mut self) {
        if let Some(tab) = self.active_tab() {
            tab.copy_picked();
        }
    }

    pub(crate) fn cut_figures(&mut self) {
        self.copy_figures();
        self.delete_figures();
    }

    pub(crate) fn paste_figures(&mut self) {
        self.edit(Tab::paste_figures);
        self.adopt_picked();
    }

    pub(crate) fn duplicate_figures(&mut self) {
        self.edit(Tab::duplicate_picked);
    }

    pub(crate) fn stack_figures(&mut self, to: StackOrder) {
        self.edit(|tab| tab.stack_picked(to));
    }

    pub(crate) fn rasterize_figures(&mut self) {
        self.edit(Tab::rasterize_figures);
    }

    pub(crate) fn edit_text_figure(&mut self, figure: Figure) {
        if self.unsaved_prompt.is_some() {
            return;
        }
        self.adopt(adopted(&figure.form));
        self.tool = Tool::Text;
        self.edit(|tab| tab.edit_text(&figure));
    }

    pub(crate) fn drop_text(&mut self) {
        self.edit(Tab::drop_text);
    }

    pub(crate) fn open_figure_menu(&mut self, at: Point) {
        self.edit(|tab| tab.open_figure_menu(at));
    }

    pub(crate) fn close_figure_menu(&mut self) {
        self.edit(Tab::close_figure_menu);
    }

    pub(super) fn restyle(&mut self, restyle: Restyle, commit: bool) {
        if !self.tool.picks() || self.unsaved_prompt.is_some() {
            return;
        }
        self.edit(|tab| tab.restyle_picked(&restyle, commit));
    }

    fn adopt_picked(&mut self) {
        let figure = self
            .active_tab()
            .and_then(|tab| tab.picked_figures().last().cloned());
        if let Some(figure) = figure {
            self.adopt(adopted(&figure.form));
        }
    }

    fn adopt(&mut self, adopted: StyleSource) {
        self.colors.primary = adopted.primary;
        if let Some(secondary) = adopted.secondary {
            self.colors.secondary = secondary;
        }
        if let Some(width) = adopted.width {
            self.options.set_width(adopted.tool, width);
        }
        if let Some(filling) = adopted.filling {
            self.options.filling = filling;
        }
        if let Some(antialiased) = adopted.antialiased {
            self.options.antialiased = antialiased;
        }
        if let Some(letters) = adopted.letters {
            self.options.letters = letters;
        }
    }
}
