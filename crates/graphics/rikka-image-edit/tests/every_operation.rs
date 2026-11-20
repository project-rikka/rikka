use crate::support::{imaging, picture};
use rikka_image_edit::*;
use rikka_types::{Color, Point};

fn operations() -> Vec<Operation> {
    let mut pen = Mark::new(
        Tool::Pen,
        Color::rgb(0.2, 0.6, 0.9),
        3.0,
        Point::new(2.0, 2.0),
    );
    pen.extend_to(Point::new(20.0, 12.0));
    vec![
        Levels::new()
            .channel(Channel::Red, Level::new().gamma(1.4))
            .into(),
        Curves::new()
            .channel(Channel::Composite, [(0.0, 0.1), (0.5, 0.6), (1.0, 0.9)])
            .into(),
        BrightnessContrast::new(20.0, 30.0).into(),
        BrightnessContrast::new(20.0, 30.0).legacy(true).into(),
        Exposure::new(-0.5).offset(0.01).gamma(1.1).into(),
        Posterize::new(6).into(),
        Invert.into(),
        Threshold::new(0.4).into(),
        HueSaturation::new()
            .hues(Hues::Blues, Shift::new(20.0, 10.0, -5.0))
            .into(),
        ColorBalance::new()
            .shadows(10.0, -5.0, 20.0)
            .highlights(-10.0, 0.0, 5.0)
            .into(),
        BlackWhite::new()
            .weight(Hues::Reds, 90.0)
            .tint(Color::rgb(0.9, 0.8, 0.6))
            .into(),
        Sepia::new().into(),
        PhotoFilter::default().into(),
        ChannelMixer::new()
            .monochrome([40.0, 40.0, 20.0], 0.0)
            .into(),
        GradientMap::default().reversed(true).into(),
        SelectiveColor::new()
            .colors(Colors::Neutrals, 0.0, 10.0, -10.0, 5.0)
            .absolute(true)
            .into(),
        Vibrance::new(40.0).into(),
        Desaturate::Average.into(),
        WhiteBalance::new(9000.0).tint(20.0).into(),
        AutoLevels::new().into(),
        AutoContrast::new().into(),
        AutoColor::new().into(),
        Equalize.into(),
        GaussianBlur::new(1.5).into(),
        GaussianBlur::new(40.0).into(),
        BoxBlur::new(3.0).into(),
        MotionBlur::new(30.0, 6.0).into(),
        RadialBlur::spin(20.0).quality(Quality::Draft).into(),
        RadialBlur::zoom(30.0).center(0.3, 0.6).into(),
        LensBlur::new(20.0)
            .iris(5, 10.0, 0.5)
            .specular(30.0, 0.8)
            .into(),
        SurfaceBlur::new(3.0, 20.0).into(),
        Median::new(2.0).into(),
        TiltShift::new(6.0).line(0.4, 10.0).band(0.1, 0.2).into(),
        UnsharpMask::new(120.0, 1.5).threshold(2.0).into(),
        SmartSharpen::new(150.0, 1.0).into(),
        SmartSharpen::new(150.0, 1.0)
            .removal(Removal::GaussianBlur)
            .reduce_noise(0.0)
            .into(),
        SmartSharpen::new(150.0, 1.0)
            .removal(Removal::MotionBlur { angle: 30.0 })
            .into(),
        HighPass::new(4.0).into(),
        Clarity::new(-30.0).into(),
        AddNoise::new(10.0)
            .distribution(Distribution::Uniform)
            .monochromatic(true)
            .into(),
        ReduceNoise::new(5.0).into(),
        Emboss::default().into(),
        FindEdges.into(),
        Mosaic::new(5.0).into(),
        OilPaint::new().into(),
        Vignette::new(40.0).roundness(-50.0).into(),
        Glow::new(4.0).into(),
        Dither::new(4).method(Dithering::Ordered).into(),
        Dither::new(3).into(),
        ShadowsHighlights::new(50.0, 30.0)
            .radius(6.0)
            .midtones(20.0)
            .into(),
        Resize::new(30, 20).into(),
        Rotate::new(17.0).fill(Color::WHITE).into(),
        QuarterTurn::Clockwise.into(),
        Flip::Vertical.into(),
        Crop::new(2, 3, 10, 8).into(),
        CanvasSize::new(40, 30).into(),
        Fill::new(Color::rgba(0.2, 0.4, 0.6, 0.5)).into(),
        Transform::new([[1.0, 0.2, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], 30, 20).into(),
        ConvertProfile::new(Profile::adobe_rgb())
            .intent(Intent::Perceptual)
            .into(),
        ConvertProfile::new(Profile::pro_photo_rgb())
            .intent(Intent::AbsoluteColorimetric)
            .into(),
        Marks::new([pen]).into(),
        Stamp::new(
            2,
            2,
            vec![255, 0, 0, 255, 0, 0, 255, 128, 0, 0, 0, 0, 9, 9, 9, 9],
        )
        .expect("valid stamp")
        .with_position(3, 4)
        .into(),
        Marks::new([
            Marking::from(
                BrushStroke::from_points(
                    Brush {
                        hardness: 0.3,
                        flow: 0.4,
                        ..Brush::new(6.0)
                    },
                    Ink::Erase {
                        background: Color::WHITE,
                    },
                    [Point::new(3.0, 3.0), Point::new(18.0, 14.0)],
                )
                .expect("stroke has points"),
            ),
            Marking::from(
                PathMark::new(Path::polygon([
                    Point::new(2.0, 2.0),
                    Point::new(20.0, 4.0),
                    Point::new(8.0, 16.0),
                ]))
                .filled(Color::rgb(0.9, 0.3, 0.1), FillRule::EvenOdd)
                .stroked(Color::BLACK, StrokeStyle::new(2.0).join(Join::Round)),
            ),
        ])
        .into(),
        Gradient::new(
            GradientShape::Conical,
            Point::new(4.0, 4.0),
            Point::new(20.0, 10.0),
            GradientInk::Colors {
                from: Color::BLACK,
                to: Color::rgba(1.0, 1.0, 1.0, 0.5),
            },
        )
        .into(),
        Place::new(
            picture(6, 5, |x, y| [(x * 40) as u8, (y * 50) as u8, 90, 200]),
            [[0.7, 0.3, 4.0], [-0.3, 0.7, 6.0], [0.0, 0.0, 1.0]],
        )
        .radius(2.0)
        .blend(Blend::Screen)
        .opacity(0.8)
        .into(),
        Place::new(
            picture(40, 30, |x, y| [(x * 6) as u8, (y * 8) as u8, 30, 255]),
            [[0.2, 0.0, 2.0], [0.0, 0.25, 1.0], [0.004, 0.0, 1.0]],
        )
        .resampling(Resampling::Bicubic)
        .into(),
        Place::new(
            picture(3, 3, |x, _| [(x * 100) as u8, 0, 0, 255]),
            [[-4.0, 0.0, 20.0], [0.0, 4.0, 2.0], [0.0, 0.0, 1.0]],
        )
        .resampling(Resampling::Nearest)
        .blend(Blend::Dissolve)
        .into(),
        ChromaKey::new(Color::rgb(0.0, 1.0, 0.0)).into(),
        Outline::new(Color::WHITE).width(2.0).softness(2.0).into(),
        Place::new(
            picture(6, 5, |x, y| [(x * 40) as u8, (y * 50) as u8, 90, 200]),
            [[2.0, 0.0, -3.0], [0.0, 2.0, -2.0], [0.0, 0.0, 1.0]],
        )
        .replacing()
        .mirrored()
        .into(),
        FractalNoise::new(6.0)
            .detail(3)
            .seed(7)
            .evolution(0.4)
            .into(),
        Displace::new(
            picture(5, 4, |x, y| [(x * 60) as u8, (y * 70) as u8, 0, 255]),
            4.0,
            -3.0,
        )
        .into(),
        LensDistortion::new(60.0).center(0.4, 0.6).into(),
        ChannelShift::new(30.0, 3.0).into(),
        Clipping::new(2.0, 1.0, 3.0, 4.0).feather(2.0).into(),
        DropShadow::new(Color::BLACK).into(),
        Outline::new(Color::WHITE).width(2.0).softness(2.0).into(),
    ]
}

#[test]
fn every_operation_develops_on_every_depth() {
    let eight = picture(24, 18, |x, y| {
        [(x * 10) as u8, (y * 14) as u8, ((x + y) * 5) as u8, 255]
    });
    let sixteen = imaging()
        .picture(
            &eight
                .to_image()
                .unwrap()
                .convert(rikka_image::ColorType::Rgba16),
        )
        .unwrap();
    let float = imaging()
        .picture(
            &eight
                .to_image()
                .unwrap()
                .convert(rikka_image::ColorType::Rgba32F),
        )
        .unwrap();
    for source in [eight, sixteen, float] {
        for operation in operations() {
            let developed = source
                .develop(&Recipe::new().then(operation.clone()))
                .unwrap_or_else(|error| {
                    panic!("{operation:?} on {:?} failed: {error}", source.depth())
                });
            assert_eq!(developed.depth(), source.depth());
            developed.to_image().unwrap();
        }
    }
}

#[test]
fn every_operation_previews_at_half_scale() {
    let source = picture(48, 32, |x, y| [(x * 5) as u8, (y * 7) as u8, 128, 255]);
    let mut preview = Preview::new(source);
    for operation in operations() {
        preview
            .render(&Recipe::new().then(operation.clone()), 0.5)
            .unwrap_or_else(|error| panic!("{operation:?} failed to preview: {error}"));
    }
}
