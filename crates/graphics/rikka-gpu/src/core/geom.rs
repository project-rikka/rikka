use ash::vk;
use rikka_types::Rect;

pub fn rect_to_viewport(rect: Rect) -> vk::Viewport {
    vk::Viewport {
        x: rect.min_x(),
        y: rect.min_y(),
        width: rect.width(),
        height: rect.height(),
        min_depth: 0.0,
        max_depth: 1.0,
    }
}

pub fn rect_to_vk(rect: Rect) -> vk::Rect2D {
    let rounded = rect.round_out();
    let x = rounded.min_x().max(0.0);
    let y = rounded.min_y().max(0.0);
    vk::Rect2D {
        offset: vk::Offset2D {
            x: x as i32,
            y: y as i32,
        },
        extent: vk::Extent2D {
            width: (rounded.max_x() - x).max(0.0) as u32,
            height: (rounded.max_y() - y).max(0.0) as u32,
        },
    }
}
