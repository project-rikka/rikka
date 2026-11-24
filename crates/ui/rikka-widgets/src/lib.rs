mod blocks;
mod control;
mod controls;
mod dragging;
mod dsl;
mod lasting;
mod lists;
mod media;
mod navigation;
mod rows;
mod sums;
mod text;

pub use control::ControlSize;
pub use dragging::drag_chip;

pub use blocks::animated_position::{AnimatedPosition, animated_position};
pub use blocks::async_view::{AsyncView, async_view};
pub use blocks::banner::{Banner, banner};
pub use blocks::collapsible::{Collapsible, collapsible};
pub use blocks::container::{Container, container};
pub use blocks::deferred::{Deferred, deferred};
pub use blocks::dialog::{Dialog, dialog};
pub use blocks::dismissible::{Dismissible, dismissible};
pub use blocks::drop_zone::{DropZone, drop_zone};
pub use blocks::flex::{Flex, column_of, grid_of, row_of};
pub use blocks::gliding::{Gliding, gliding};
pub use blocks::lazy::{Lazy, lazy};
pub use blocks::live::{Live, live, live_observed};
pub use blocks::page::{Field, card, field, page, section};
pub use blocks::popover::{Popover, popover};
pub use blocks::rule::{rule, rule_upright};
pub use blocks::scaled::{Scaled, scaled};
pub use blocks::scrim::{Scrim, scrim};
pub use blocks::scrollable::{Scrollable, scrollable};
pub use blocks::sheet::{Sheet, Unsaved, alert_prompt, confirm_prompt, sheet, unsaved_prompt};
pub use blocks::shortcuts::{Shortcuts, shortcuts};
pub use blocks::spacer::{Spacer, spacer};
pub use blocks::status_page::{StatusPage, status_page};

pub use controls::button::{Button, ButtonVariant, button};
pub use controls::checkbox::{CheckState, Checkbox, checkbox};
pub use controls::color::picker::{ColorPicker, color_picker};
pub use controls::color::well::{ColorWell, color_well};
pub use controls::copy_button::{CopyButton, copy_button};
pub use controls::gauge::{Gauge, gauge};
pub use controls::grab::{Grab, GrabStep};
pub use controls::live_progress::{LiveProgress, live_progress};
pub use controls::node_canvas;
pub use controls::node_canvas::{NodeCanvas, node_canvas};
pub use controls::progress::{Progress, progress, progress_indeterminate};
pub use controls::radio::{Radio, radio};
pub use controls::segmented::{Segmented, segmented};
pub use controls::slider::{Slider, slider};
pub use controls::spinner::{Spinner, spinner};
pub use controls::switch::{Switch, switch};
pub use controls::tool_tile::tool_tile;

pub use lists::collection::{
    Collection, Highlights, Slot, Surface, lazy_column, lazy_grid, lazy_row, lazy_tiles,
};
pub use lists::columns::{
    COLUMN_HEADER, COLUMN_PADDING, ColumnHeader, ColumnSpec, ColumnWidths, column_header,
    column_spec,
};
pub use lists::jump_to_end::{JumpToEnd, jump_to_end};
pub use lists::list::{List, list, list_of};
pub use lists::list_item::{ListItem, list_header, list_item};
pub use lists::nav_list::{NavItem, NavList, nav_header, nav_item, nav_list};
pub use lists::twistie::{Twistie, paint_twistie, twistie};

pub use media::fit::ImageFit;
pub use media::icon::{Icon, icon};
pub use media::image::{Image, image};
pub use media::pan_zoom::{PanZoom, Zoom};
pub use media::video::video;

pub use navigation::caption::{Caption, caption};
pub use navigation::combo::{Combo, combo};
pub use navigation::context_area::{ContextArea, context_area};
pub use navigation::dropdown::{Dropdown, dropdown};
pub use navigation::menu::{
    Activation, ContextMenu, Menu, MenuItem, command_item, command_label, menu, menu_header,
    menu_item, menu_separator, menu_slider,
};
pub use navigation::menu_bar::{MenuBar, menu_bar};
pub use navigation::page_stack::{PageStack, page_stack};
pub use navigation::panes::{
    DropTarget, Pane, Ratio, SPLIT_GUTTER, Side, Split, SplitRatios, pane, split,
};
pub use navigation::split_view::{SplitView, split_view};
pub use navigation::tabs::{Tab, Tabs, Transfer, tab, tabs};
pub use navigation::title_bar::CaptionBar;
pub use navigation::toolbar::{
    ToolButton, ToolGroup, ToolItem, ToolPriority, Toolbar, tool_button, tool_group, tool_item,
    toolbar,
};
pub use navigation::window_frame::{WindowFrame, window_frame};

pub use text::editor::{Edit, Edits, TextEditor, text_editor};
pub use text::find_bar::{FindBar, find_bar, find_mark};
pub use text::fitting::{FittedLine, fitted_line, fitted_lines};
pub use text::menu::text_menu;
pub use text::number_field::{NumberField, number_field};
pub use text::rich_text::{RichText, SpanFill, rich_text};
pub use text::shortcut_field::{ShortcutField, shortcut_field};
pub use text::text_field::{TextField, text_field};

#[doc(hidden)]
pub use dsl::{__children, Children};

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();

pub use crate::__column as column;
pub use crate::__page as page;
pub use crate::__row as row;
