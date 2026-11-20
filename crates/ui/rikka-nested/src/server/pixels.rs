use rikka_compositor_types::Fourcc;

pub(crate) fn rgba(
    format: Fourcc,
    width: usize,
    height: usize,
    stride: usize,
    pixels: &[u8],
) -> Option<Vec<u8>> {
    let opaque = match format {
        Fourcc::ARGB8888 => false,
        Fourcc::XRGB8888 => true,
        _ => return None,
    };
    if stride < width * 4 || pixels.len() < stride * height.saturating_sub(1) + width * 4 {
        return None;
    }
    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in pixels.chunks(stride).take(height) {
        for pixel in row[..width * 4].as_chunks::<4>().0 {
            let alpha = if opaque { u8::MAX } else { pixel[3] };
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], alpha]);
        }
    }
    Some(rgba)
}
