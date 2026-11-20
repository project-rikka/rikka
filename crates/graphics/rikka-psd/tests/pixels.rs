use rikka_psd::{
    ColorConverter, ColorMode, ColorModeData, Compression, Depth, Document, Encoded, FileHeader,
    Layer, Node, Plane, ProfileIssue, ResourceData, RgbaImage, Version, VersionInfo, resource_id,
};

#[test]
fn packbits_round_trips_awkward_rows() {
    let rows: [&[u8]; 6] = [
        &[],
        &[7],
        &[1, 1],
        &[1, 2, 3, 3, 3, 3, 4, 5, 5],
        &[9; 300],
        &[
            0, 1, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        ],
    ];
    for row in rows {
        let width = row.len() as u32;
        if width == 0 {
            continue;
        }
        let plane = Plane::from_bytes(width, 1, Depth::Eight, row).unwrap();
        let encoded = Encoded::encode(&plane, Compression::Rle);
        assert_eq!(encoded.decode(width, 1, Depth::Eight).unwrap(), plane);
    }
}

#[test]
fn every_compression_round_trips_every_depth() {
    for depth in [Depth::One, Depth::Eight, Depth::Sixteen, Depth::ThirtyTwo] {
        let values: Vec<f32> = (0..37 * 11)
            .map(|index| ((index * 31) % 101) as f32 / 100.0)
            .collect();
        let plane = Plane::from_normalized(37, 11, depth, &values).unwrap();
        for compression in [
            Compression::Raw,
            Compression::Rle,
            Compression::Zip,
            Compression::ZipPredicted,
        ] {
            let encoded = Encoded::encode(&plane, compression);
            assert_eq!(
                encoded.decode(37, 11, depth).unwrap(),
                plane,
                "{depth:?} {compression:?}"
            );
        }
    }
}

#[test]
fn transparent_composite_is_matted_and_unmatted() {
    let mut document = Document::new(2, 1, Depth::Eight, ColorMode::Rgb);
    let image = RgbaImage::from_rgba8(2, 1, &[255, 0, 0, 128, 0, 0, 255, 255]).unwrap();
    document
        .set_composite_rgba(&image, Compression::Rle)
        .unwrap();
    assert!(document.merged_alpha);
    let planes = document.composite_planes().unwrap();
    assert_eq!(planes.len(), 4);
    assert!((planes[1].normalized(0, 0) - (1.0 - 128.0 / 255.0)).abs() < 0.01);
    let read = Document::read(&document.to_bytes().unwrap()).unwrap();
    let rgba = read.composite_rgba().unwrap().to_rgba8();
    assert!(rgba[0] >= 253 && rgba[1] <= 2 && rgba[3] == 128);
    assert_eq!(&rgba[4..], &[0, 0, 255, 255]);
}

#[test]
fn layer_pixels_round_trip_through_cmyk() {
    let document = Document::new(3, 1, Depth::Eight, ColorMode::Cmyk);
    let mut layer = Layer::new("ink");
    let image =
        RgbaImage::from_rgba8(3, 1, &[255, 255, 255, 255, 0, 0, 0, 255, 0, 255, 255, 64]).unwrap();
    document
        .set_layer_rgba(&mut layer, 0, 0, &image, Compression::Rle)
        .unwrap();
    assert_eq!(layer.channels.len(), 5);
    let back = document.layer_rgba(&layer).unwrap().to_rgba8();
    for (got, want) in back.iter().zip(image.to_rgba8()) {
        assert!(got.abs_diff(want) <= 2, "{back:?}");
    }
}

fn cmyk_converter(icc: Option<Vec<u8>>) -> ColorConverter {
    let mut document = Document::new(1, 1, Depth::Eight, ColorMode::Cmyk);
    if let Some(icc) = icc {
        document
            .resources
            .set(resource_id::ICC_PROFILE, ResourceData::IccProfile(icc));
    }
    document.color_converter()
}

#[test]
fn unusable_cmyk_profile_is_reported_and_falls_back() {
    let srgb = moxcms::ColorProfile::new_srgb()
        .encode()
        .expect("sRGB profile encodes");
    let unreadable = cmyk_converter(Some(vec![0; 16]));
    let not_cmyk = cmyk_converter(Some(srgb));

    assert!(matches!(
        unreadable.profile_issue(),
        Some(ProfileIssue::Unreadable(_))
    ));
    assert_eq!(not_cmyk.profile_issue(), Some(&ProfileIssue::NotCmyk));
    assert_eq!(cmyk_converter(None).profile_issue(), None);
    for converter in [unreadable, not_cmyk] {
        assert!(!converter.is_managed());
        let white = converter.to_rgb(&[vec![1.0], vec![1.0], vec![1.0], vec![1.0]]);
        assert_eq!(white, vec![[1.0, 1.0, 1.0]]);
    }
}

#[test]
fn indexed_and_bitmap_composites_decode() {
    let mut indexed = Document::new(2, 1, Depth::Eight, ColorMode::Indexed);
    let mut palette = vec![[0u8; 3]; 256];
    palette[1] = [10, 20, 30];
    indexed.color_mode_data = ColorModeData::Palette(palette);
    indexed
        .set_composite(
            &[Plane::from_bytes(2, 1, Depth::Eight, &[1, 0]).unwrap()],
            Compression::Rle,
        )
        .unwrap();
    let read = Document::read(&indexed.to_bytes().unwrap()).unwrap();
    assert_eq!(
        read.composite_rgba().unwrap().to_rgba8(),
        vec![10, 20, 30, 255, 0, 0, 0, 255]
    );

    let mut bitmap = Document::new(10, 1, Depth::One, ColorMode::Bitmap);
    bitmap
        .set_composite(
            &[Plane::from_bytes(10, 1, Depth::One, &[0b1000_0000, 0b0100_0000]).unwrap()],
            Compression::Raw,
        )
        .unwrap();
    let rgba = Document::read(&bitmap.to_bytes().unwrap())
        .unwrap()
        .composite_rgba()
        .unwrap()
        .to_rgba8();
    assert_eq!(rgba[0], 0);
    assert_eq!(rgba[4], 255);
    assert_eq!(rgba[9 * 4], 0);
}

#[test]
fn composite_without_merged_data_is_not_real() {
    let mut document = Document::new(4, 4, Depth::Eight, ColorMode::Rgb);
    document
        .set_composite_rgba(&RgbaImage::new(4, 4), Compression::Rle)
        .unwrap();
    assert!(document.has_real_composite());

    document.layers.push(Node::Layer(Layer::new("ink")));
    document.resources.set(
        resource_id::VERSION_INFO,
        ResourceData::VersionInfo(VersionInfo {
            version: 1,
            has_real_merged_data: false,
            writer: String::from("Adobe Photoshop"),
            reader: String::from("Adobe Photoshop"),
            file_version: 1,
        }),
    );
    assert!(!document.has_real_composite());

    let empty = Document::new(4, 4, Depth::Eight, ColorMode::Cmyk);
    assert!(!empty.has_real_composite());
    assert!(empty.composite_rgba8().is_err());
}

#[test]
fn header_reads_without_body() {
    let document = Document::new(640, 480, Depth::Sixteen, ColorMode::Cmyk);
    let bytes = document.to_bytes().unwrap();

    let header = FileHeader::read(&bytes[..FileHeader::LEN]).unwrap();

    assert_eq!((header.width, header.height), (640, 480));
    assert_eq!(header.depth, Depth::Sixteen);
    assert_eq!(header.color_mode, ColorMode::Cmyk);
    assert_eq!(header.version, Version::Psd);
    assert!(FileHeader::read(b"8BPS").is_err());
    assert!(FileHeader::read(&[0; FileHeader::LEN]).is_err());
}
