use rikka_types::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wipe {
    Up,
    Down,
    Left,
    Right,
}

pub(crate) fn revealed(direction: Wipe, bounds: Rect, progress: f32) -> Rect {
    let progress = progress.clamp(0.0, 1.0);
    let (width, height) = (bounds.width(), bounds.height());
    match direction {
        Wipe::Down => Rect::new(bounds.x(), bounds.y(), width, height * progress),
        Wipe::Up => {
            let shown = height * progress;
            Rect::new(bounds.x(), bounds.max_y() - shown, width, shown)
        }
        Wipe::Right => Rect::new(bounds.x(), bounds.y(), width * progress, height),
        Wipe::Left => {
            let shown = width * progress;
            Rect::new(bounds.max_x() - shown, bounds.y(), shown, height)
        }
    }
}
