use rikka_types::Rect;

const LARGEST: f32 = 0.95;

const SCALE_WEIGHT: f32 = 1.0;

const SPACE_WEIGHT: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    pub row: f32,
    pub column: f32,
}

#[derive(Debug)]
struct Row {
    windows: Vec<usize>,
    full_width: f32,
    full_height: f32,
}

#[derive(Debug)]
pub(crate) struct Layout {
    rows: Vec<Row>,
    columns: usize,
    width: f32,
    height: f32,
}

#[must_use]
pub fn arrange(windows: &[Rect], area: Rect, spacing: Spacing, screen_height: f32) -> Vec<Rect> {
    if windows.is_empty() || area.width() <= 0.0 || area.height() <= 0.0 {
        return Vec::new();
    }
    let grown: Vec<f32> = windows
        .iter()
        .map(|window| grown(*window, screen_height))
        .collect();
    let mut best: Option<(Layout, f32, f32)> = None;
    let mut columns_before = 0;
    for rows in 1..=windows.len() {
        let columns = windows.len().div_ceil(rows);
        if columns == columns_before {
            break;
        }
        columns_before = columns;
        let layout = lay(windows, &grown, rows);
        let (scale, space) = scale_and_space(&layout, area, spacing);
        if let Some((_, held_scale, held_space)) = &best
            && !better(*held_scale, *held_space, scale, space)
        {
            break;
        }
        best = Some((layout, scale, space));
    }
    best.map_or_else(Vec::new, |(layout, scale, _)| {
        slots(&layout, windows, &grown, scale, area, spacing)
    })
}

fn grown(window: Rect, tall: f32) -> f32 {
    let ratio = (window.height() / tall.max(1.0)).clamp(0.0, 1.0);
    1.5 + (1.0 - 1.5) * ratio
}

fn lay(windows: &[Rect], grown: &[f32], rows: usize) -> Layout {
    let total: f32 = windows
        .iter()
        .zip(grown)
        .map(|(window, grown)| window.width() * grown)
        .sum();
    let ideal = total / rows as f32;
    let mut sorted: Vec<usize> = (0..windows.len()).collect();
    sorted.sort_by(|a, b| windows[*a].center().y.total_cmp(&windows[*b].center().y));

    let mut laid: Vec<Row> = Vec::with_capacity(rows);
    let mut next = sorted.into_iter().peekable();
    for row in 0..rows {
        let mut current_row = Row {
            windows: Vec::new(),
            full_width: 0.0,
            full_height: 0.0,
        };
        while let Some(&index) = next.peek() {
            let width = windows[index].width() * grown[index];
            let height = windows[index].height() * grown[index];
            current_row.full_height = current_row.full_height.max(height);
            if keeps(&current_row, width, ideal) || row == rows - 1 {
                current_row.windows.push(index);
                current_row.full_width += width;
                next.next();
            } else {
                break;
            }
        }
        laid.push(current_row);
    }
    for row in &mut laid {
        row.windows
            .sort_by(|a, b| windows[*a].center().x.total_cmp(&windows[*b].center().x));
    }
    let widest = laid
        .iter()
        .max_by(|a, b| a.full_width.total_cmp(&b.full_width));
    Layout {
        columns: widest.map_or(0, |row| row.windows.len()),
        width: widest.map_or(0.0, |row| row.full_width),
        height: laid.iter().map(|row| row.full_height).sum(),
        rows: laid,
    }
}

fn keeps(row: &Row, width: f32, ideal: f32) -> bool {
    if row.full_width + width <= ideal {
        return true;
    }
    let before = row.full_width / ideal;
    let after = (row.full_width + width) / ideal;
    (1.0 - after).abs() < (1.0 - before).abs()
}

fn scale_and_space(layout: &Layout, area: Rect, spacing: Spacing) -> (f32, f32) {
    let across = layout.columns.saturating_sub(1) as f32 * spacing.column;
    let down = layout.rows.len().saturating_sub(1) as f32 * spacing.row;
    let wide = (area.width() - across) / layout.width.max(1.0);
    let tall = (area.height() - down) / layout.height.max(1.0);
    let scale = wide.min(tall).min(LARGEST);
    let occupied = (layout.width * scale + across) * (layout.height * scale + down);
    (scale, occupied / (area.width() * area.height()))
}

fn better(held_scale: f32, held_space: f32, scale: f32, space: f32) -> bool {
    if scale > held_scale && space > held_space {
        return true;
    }
    if scale < held_scale && space < held_space {
        return false;
    }
    (scale - held_scale) * SCALE_WEIGHT + (space - held_space) * SPACE_WEIGHT > 0.0
}

fn slots(
    layout: &Layout,
    windows: &[Rect],
    grown: &[f32],
    scale: f32,
    area: Rect,
    spacing: Spacing,
) -> Vec<Rect> {
    let rows = &layout.rows;
    let width_of = |row: &Row| row.full_width * scale;
    let height_of = |row: &Row| row.full_height * scale;
    let bare_height: f32 = rows.iter().map(height_of).sum();
    let down = rows.len().saturating_sub(1) as f32 * spacing.row;
    let vertical = ((area.height() - down) / bare_height.max(1.0)).min(1.0);

    let mut compensation = 0.0;
    let mut placed_rows = Vec::with_capacity(rows.len());
    let mut y = 0.0;
    for row in rows {
        let across = row.windows.len().saturating_sub(1) as f32 * spacing.column;
        let bare_width = width_of(row);
        let horizontal = ((area.width() - across) / bare_width.max(1.0)).min(1.0);
        let extra = match horizontal < vertical {
            true => {
                compensation += (vertical - horizontal) * height_of(row);
                horizontal
            }
            false => vertical,
        };
        let x = area.min_x() + (area.width() - (bare_width * extra + across)).max(0.0) / 2.0;
        let top = area.min_y() + (area.height() - (bare_height + down)).max(0.0) / 2.0 + y;
        y += height_of(row) * extra + spacing.row;
        placed_rows.push((x, top, extra));
    }
    compensation /= 2.0;

    let mut slots = vec![Rect::ZERO; windows.len()];
    for (index, (row, (x, top, extra))) in rows.iter().zip(placed_rows).enumerate() {
        let row_top = top + compensation;
        let row_height = height_of(row) * extra;
        let mut left = x;
        for &window in &row.windows {
            let cell = scale * grown[window] * extra;
            let cell_width = windows[window].width() * cell;
            let shown = cell.min(LARGEST);
            let width = windows[window].width() * shown;
            let height = windows[window].height() * shown;
            let top = match (rows.len(), index) {
                (1, _) => row_top + (row_height - height) / 2.0,
                (_, 0) => row_top + row_height - height,
                _ => row_top,
            };
            slots[window] = Rect::new(left + (cell_width - width) / 2.0, top, width, height);
            left += cell_width + spacing.column;
        }
    }
    slots
}
