use rikka_compositor_geometry::{Physical, Rect, Region};

pub(crate) fn hide(
    covered: &mut Region<Physical>,
    opaque: &[Rect<i32, Physical>],
    stack: usize,
    bands: &[(usize, Rect<i32, Physical>)],
) {
    let mut over = bands
        .iter()
        .filter(|(under, _)| *under <= stack)
        .map(|(_, band)| *band)
        .peekable();
    if over.peek().is_none() {
        for rect in opaque {
            covered.add(*rect);
        }
        return;
    }
    let mut hidden: Region<Physical> = opaque.iter().copied().collect();
    for band in over {
        hidden.subtract(band);
    }
    covered.union(&hidden);
}
