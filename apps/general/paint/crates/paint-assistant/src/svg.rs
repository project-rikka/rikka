use rikka_image_edit::Path;
use rikka_path_data::{Error, ErrorKind, Segment, segments};

const MAX_SEGMENTS: usize = 100_000;
const COMMANDS: &str = "M, L, H, V, C, S, Q, T, A or Z";

pub fn path(d: &str) -> Result<Path, String> {
    if d.trim().is_empty() {
        return Err("the path is empty".to_owned());
    }
    let mut builder = Path::builder();
    for (count, segment) in segments(d).enumerate() {
        if count >= MAX_SEGMENTS {
            return Err(format!(
                "the path has more than {MAX_SEGMENTS} segments; simplify it"
            ));
        }
        builder = match segment.map_err(|error| failure(d, error))? {
            Segment::MoveTo(to) => builder.move_to(to),
            Segment::LineTo(to) => builder.line_to(to),
            Segment::QuadTo(control, to) => builder.quad_to(control, to),
            Segment::CubicTo(first, second, to) => builder.cubic_to(first, second, to),
            Segment::Close => builder.close(),
        };
    }
    let built = builder.build();
    match built.is_empty() {
        true => Err("the path draws nothing; it needs a line or curve after its M".to_owned()),
        false => Ok(built),
    }
}

#[must_use]
pub fn outlines(path: &Path) -> Vec<Vec<(f32, f32)>> {
    path.subpaths()
        .iter()
        .map(|subpath| {
            subpath
                .points()
                .iter()
                .map(|point| (point.x, point.y))
                .collect()
        })
        .collect()
}

fn failure(d: &str, error: Error) -> String {
    let at = d[..error.offset].chars().count() + 1;
    let found = match d[error.offset..].chars().next() {
        Some(found) => format!("'{found}'"),
        None => "the end".to_owned(),
    };
    match error.kind {
        ErrorKind::UnexpectedEnd => {
            "the path ends before its last command has all its numbers".to_owned()
        }
        ErrorKind::NoMoveFirst => {
            format!("at character {at}: a path starts with M or m, not {found}")
        }
        ErrorKind::UnknownCommand(_) => format!(
            "at character {at}: {found} does not belong here; a path is commands ({COMMANDS}) each followed by its numbers"
        ),
        ErrorKind::Flag => {
            format!("at character {at}: expected an arc flag 0 or 1, found {found}")
        }
        ErrorKind::Number => format!(
            "at character {at}: expected a number, found {found}; numbers are plain decimals and not too large"
        ),
    }
}
