use crate::support::{imaging, rgba};

fn gradient(width: u32, height: u32) -> Vec<u8> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| [(x % 251) as u8, (y * 40) as u8, (x / 251) as u8, 255])
        .collect()
}

#[test]
fn wide_upload_splits_tiles_without_row_shift() {
    let pixels = gradient(4100, 3);

    let uploaded = imaging()
        .upload_rgba8(4100, 3, &pixels)
        .expect("valid picture");

    assert_eq!(rgba(&uploaded), pixels);
}

#[test]
fn rejects_wrong_pixel_length() {
    assert!(imaging().upload_rgba8(4, 4, &[0; 60]).is_err());
}

#[test]
fn repeated_uploads_stay_correct() {
    let imaging = imaging();
    for frame in 0..24u8 {
        let pixels: Vec<u8> = (0..16 * 9)
            .flat_map(|_| [frame, 255 - frame, 7, 255])
            .collect();
        let uploaded = imaging.upload_rgba8(16, 9, &pixels).expect("valid picture");
        assert_eq!(rgba(&uploaded), pixels, "frame {frame}");
    }
}
