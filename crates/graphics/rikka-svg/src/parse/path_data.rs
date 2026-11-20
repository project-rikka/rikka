use rikka_path_data::{Segment, segments};
use rikka_vector::Path;

pub(crate) fn path_data(text: &str) -> Path {
    let mut path = Path::new();
    for segment in segments(text).map_while(Result::ok) {
        match segment {
            Segment::MoveTo(to) => path.move_to(to),
            Segment::LineTo(to) => path.line_to(to),
            Segment::QuadTo(control, to) => path.quad_to(control, to),
            Segment::CubicTo(first, second, to) => path.cubic_to(first, second, to),
            Segment::Close => path.close(),
        }
    }
    path
}
