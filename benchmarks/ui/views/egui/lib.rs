use egui::{
    Align, Align2, Button, Color32, CornerRadius, FontId, Label, Layout, Rect, RichText,
    ScrollArea, Sense, Ui, UiBuilder, Vec2, vec2,
};
use ui_scene::{
    BACKGROUND, CELL_RADIUS, GRID_GAP, Grid, REMOVE_WIDTH, ROW_BACKGROUND, ROW_HEIGHT, ROW_HOVERED,
    ROW_ID_WIDTH, ROW_SELECTED, ROW_TEXT_SIZE, Row, TEXT, Table, rgb,
};

pub fn color(color: u32) -> Color32 {
    let [red, green, blue] = rgb(color);
    Color32::from_rgb(red, green, blue)
}

pub fn grid(ui: &mut Ui, grid: &Grid) {
    paint_background(ui);
    let layout = grid.layout();
    let size = vec2(layout.cell_width, layout.cell_height);
    let font = FontId::proportional(layout.text_size);
    let inner = ui.max_rect().shrink(GRID_GAP);
    ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(GRID_GAP);
        ui.horizontal_wrapped(|ui| {
            for cell in grid.cells() {
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let painter = ui.painter();
                painter.rect_filled(
                    rect,
                    CornerRadius::same(CELL_RADIUS as u8),
                    color(cell.color),
                );
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    cell.label,
                    font.clone(),
                    color(TEXT),
                );
            }
        });
    });
}

pub fn table(ui: &mut Ui, table: &Table) {
    paint_background(ui);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        for row in &table.rows {
            let background = match table.is_selected(row) {
                true => ROW_SELECTED,
                false => ROW_BACKGROUND,
            };
            row_ui(ui, row, |_| background);
        }
    });
}

pub fn list(ui: &mut Ui, rows: &[Row], offset: Option<f32>) {
    paint_background(ui);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let area = ScrollArea::vertical().auto_shrink(false);
    let area = match offset {
        Some(offset) => area.vertical_scroll_offset(offset),
        None => area,
    };
    area.show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
        for row in &rows[range] {
            row_ui(ui, row, |hovered| match hovered {
                true => ROW_HOVERED,
                false => ROW_BACKGROUND,
            });
        }
    });
}

fn paint_background(ui: &mut Ui) {
    ui.painter()
        .rect_filled(ui.max_rect(), CornerRadius::ZERO, color(BACKGROUND));
}

fn row_ui(ui: &mut Ui, row: &Row, background: impl Fn(bool) -> u32) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::ZERO,
        color(background(response.hovered())),
    );
    let mut cells = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    cells.spacing_mut().item_spacing = Vec2::ZERO;
    cells.add_sized(
        [ROW_ID_WIDTH, ROW_HEIGHT],
        Label::new(row_text(row.id.to_string())),
    );
    let label_width = (rect.width() - ROW_ID_WIDTH - REMOVE_WIDTH).max(0.0);
    cells.add_sized(
        [label_width, ROW_HEIGHT],
        Label::new(row_text(row.label.clone())),
    );
    cells.add_sized(
        [REMOVE_WIDTH, ROW_HEIGHT],
        Button::new(row_text("×".to_owned())),
    );
}

fn row_text(content: String) -> RichText {
    RichText::new(content)
        .size(ROW_TEXT_SIZE)
        .color(color(TEXT))
}

pub fn full_rect(width: f32, height: f32) -> Rect {
    Rect::from_min_size(egui::Pos2::ZERO, vec2(width, height))
}
