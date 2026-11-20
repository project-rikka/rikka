use rikka_psd::RgbaImage;

pub(crate) fn numbered(width: u32, height: u32) -> RgbaImage {
    let pixels: Vec<u8> = (0..width * height)
        .flat_map(|index| [index as u8, 0, 0, 255])
        .collect();
    RgbaImage::from_rgba8(width, height, &pixels).unwrap()
}

pub(crate) fn reds(image: &RgbaImage) -> Vec<u8> {
    image
        .pixels
        .iter()
        .map(|pixel| (pixel[0] * 255.0).round() as u8)
        .collect()
}
