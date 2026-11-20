mod blur;
mod camera;
mod color;
mod detail;
mod distort;
mod glare;
mod layers;
mod stylize;
mod time;

use crate::composite::Composite;
use crate::effect::desc::{
    Effect, EffectCode, EffectFormat, EffectInput, EffectOutput, EffectSize, EffectStep,
    EffectTarget, EffectUniform,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EffectFamily {
    Blur,
    Detail,
    Glare,
    Lens,
    Film,
    Distort,
    Color,
    Stylize,
    Layers,
    Time,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Referred {
    Scene,
    Display,
}

macro_rules! builtins {
    ($($variant:ident {
        name: $name:literal,
        title: $title:literal,
        family: $family:ident,
        referred: $referred:ident,
        build: $build:ident,
        doc: $doc:literal $(,)?
    }),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum BuiltinEffect {
            $($variant,)*
        }

        impl BuiltinEffect {
            pub const ALL: [Self; [$(Self::$variant),*].len()] = [$(Self::$variant),*];

            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }

            pub fn title(self) -> &'static str {
                match self {
                    $(Self::$variant => $title,)*
                }
            }

            pub fn doc(self) -> &'static str {
                match self {
                    $(Self::$variant => $doc,)*
                }
            }

            pub fn family(self) -> EffectFamily {
                match self {
                    $(Self::$variant => EffectFamily::$family,)*
                }
            }

            pub fn effect(self) -> Effect {
                match self {
                    $(Self::$variant => Effect::$build(),)*
                }
            }

            fn referred(self) -> Referred {
                match self {
                    $(Self::$variant => Referred::$referred,)*
                }
            }
        }
    };
}

builtins! {
    Vignette {
        name: "vignette",
        title: "Vignette",
        family: Lens,
        referred: Scene,
        build: vignette,
        doc: "Darkens or tints the frame towards its edges around a movable centre, with size, softness and roundness.",
    },
    FilmGrain {
        name: "film_grain",
        title: "Film grain",
        family: Film,
        referred: Display,
        build: film_grain,
        doc: "Adds animated photographic grain sized in display space, strongest in the midtones like film.",
    },
    ChromaticAberration {
        name: "chromatic_aberration",
        title: "Chromatic aberration",
        family: Lens,
        referred: Scene,
        build: chromatic_aberration,
        doc: "Spreads the colours apart towards the frame edges like a lens's lateral chromatic aberration.",
    },
    LensDirt {
        name: "lens_dirt",
        title: "Lens dirt",
        family: Glare,
        referred: Scene,
        build: lens_dirt,
        doc: "Lights a smudged dirt pattern on the lens wherever bright light falls across it.",
    },
    Letterbox {
        name: "letterbox",
        title: "Letterbox",
        family: Stylize,
        referred: Display,
        build: letterbox,
        doc: "Masks the frame down to a wider aspect ratio with soft-edged bars.",
    },
    ColorLookup {
        name: "color_lookup",
        title: "Colour lookup",
        family: Color,
        referred: Display,
        build: color_lookup,
        doc: "Grades through a 3D lookup table or a 1D strip, with sRGB, log, ACEScct and LogC shapers for HDR input.",
    },
    ChannelMixer {
        name: "channel_mixer",
        title: "Channel mixer",
        family: Color,
        referred: Scene,
        build: channel_mixer,
        doc: "Rebuilds each colour channel as a weighted sum of red, green and blue, or mixes to monochrome.",
    },
    Hsl {
        name: "hsl",
        title: "Hue, saturation, lightness",
        family: Color,
        referred: Display,
        build: hsl,
        doc: "Shifts hue, saturation and lightness separately for eight hue bands.",
    },
    SplitToning {
        name: "split_toning",
        title: "Split toning",
        family: Color,
        referred: Display,
        build: split_toning,
        doc: "Tints the shadows and the highlights with two colours around a balance point.",
    },
    Posterize {
        name: "posterize",
        title: "Posterize",
        family: Stylize,
        referred: Display,
        build: posterize,
        doc: "Snaps every channel to a fixed number of levels.",
    },
    ColorKey {
        name: "color_key",
        title: "Colour key",
        family: Color,
        referred: Display,
        build: color_key,
        doc: "Keeps, replaces or removes one colour with tolerance, softness and spill suppression.",
    },
    Panini {
        name: "panini",
        title: "Panini projection",
        family: Distort,
        referred: Scene,
        build: panini,
        doc: "Reprojects a wide field of view so verticals stay straight and the centre keeps its proportions.",
    },
    LensDistortion {
        name: "lens_distortion",
        title: "Lens distortion",
        family: Distort,
        referred: Scene,
        build: lens_distortion,
        doc: "Bends the image with barrel or pincushion distortion and splits it by wavelength with dispersion.",
    },
    Anamorphic {
        name: "anamorphic",
        title: "Anamorphic squeeze",
        family: Lens,
        referred: Scene,
        build: anamorphic,
        doc: "Squeezes and bows the frame like an anamorphic lens before it is desqueezed.",
    },
    RadialBlur {
        name: "radial_blur",
        title: "Radial blur",
        family: Blur,
        referred: Scene,
        build: radial_blur,
        doc: "Blurs along zoom rays or spin circles around a centre, keeping an inner area sharp.",
    },
    HeatDistortion {
        name: "heat_distortion",
        title: "Heat distortion",
        family: Distort,
        referred: Scene,
        build: heat_distortion,
        doc: "Shimmers the image with animated refraction like heat haze, limited by depth and layers.",
    },
    Kuwahara {
        name: "kuwahara",
        title: "Kuwahara",
        family: Stylize,
        referred: Display,
        build: kuwahara,
        doc: "Paints the image with an anisotropic Kuwahara filter whose strokes follow its structure.",
    },
    Halftone {
        name: "halftone",
        title: "Halftone",
        family: Stylize,
        referred: Display,
        build: halftone,
        doc: "Prints the image as mono or CMYK dots or lines on paper.",
    },
    Screentone {
        name: "screentone",
        title: "Screentone",
        family: Stylize,
        referred: Display,
        build: screentone,
        doc: "Replaces tonal levels with manga screentone dots or lines.",
    },
    Sketch {
        name: "sketch",
        title: "Sketch",
        family: Stylize,
        referred: Display,
        build: sketch,
        doc: "Redraws the image as pencil hatching with depth-traced edges on paper.",
    },
    CelShade {
        name: "cel_shade",
        title: "Cel shade",
        family: Stylize,
        referred: Scene,
        build: cel_shade,
        doc: "Quantizes the luminance of chosen layers into flat bands measured in stops.",
    },
    SpeedLines {
        name: "speed_lines",
        title: "Speed lines",
        family: Stylize,
        referred: Display,
        build: speed_lines,
        doc: "Draws animated manga speed lines converging on a centre.",
    },
    Pixelate {
        name: "pixelate",
        title: "Pixelate",
        family: Stylize,
        referred: Display,
        build: pixelate,
        doc: "Reduces the image to square, round or diamond cells.",
    },
    Crt {
        name: "crt",
        title: "CRT screen",
        family: Stylize,
        referred: Display,
        build: crt,
        doc: "Shows the image on a curved CRT with scanlines, a phosphor mask, glow and misconvergence.",
    },
    Sharpen {
        name: "sharpen",
        title: "Sharpen",
        family: Detail,
        referred: Display,
        build: sharpen,
        doc: "Sharpens fine detail adaptively, without halos on strong edges.",
    },
    LayerTint {
        name: "layer_tint",
        title: "Layer tint",
        family: Layers,
        referred: Scene,
        build: layer_tint,
        doc: "Tints the objects of chosen layers, fading with depth.",
    },
    Outline {
        name: "outline",
        title: "Outline",
        family: Layers,
        referred: Display,
        build: outline,
        doc: "Traces silhouettes and creases of chosen layers from depth and normals as ink lines.",
    },
    LayerGlow {
        name: "layer_glow",
        title: "Layer glow",
        family: Layers,
        referred: Display,
        build: layer_glow,
        doc: "Highlights the objects of chosen layers with a rim, a halo and an optional fill.",
    },
    LayerBloom {
        name: "layer_bloom",
        title: "Layer bloom",
        family: Layers,
        referred: Scene,
        build: layer_bloom,
        doc: "Blooms the bright parts of chosen layers only.",
    },
    EmissionBloom {
        name: "emission_bloom",
        title: "Emission bloom",
        family: Glare,
        referred: Scene,
        build: emission_bloom,
        doc: "Blooms light from emissive surfaces only, whatever their brightness on screen.",
    },
    GaussianBlur {
        name: "gaussian_blur",
        title: "Gaussian blur",
        family: Blur,
        referred: Scene,
        build: gaussian_blur,
        doc: "Blurs with a true Gaussian of any radius at constant cost; a size image varies the radius per pixel.",
    },
    DirectionalBlur {
        name: "directional_blur",
        title: "Directional blur",
        family: Blur,
        referred: Scene,
        build: directional_blur,
        doc: "Smears the image along one direction by a length, centred or trailing.",
    },
    BokehBlur {
        name: "bokeh_blur",
        title: "Bokeh blur",
        family: Blur,
        referred: Scene,
        build: bokeh_blur,
        doc: "Convolves with a lens aperture of any blade count, squeeze or custom shape, boosting highlights; a size image varies it.",
    },
    DilateErode {
        name: "dilate_erode",
        title: "Dilate / erode",
        family: Detail,
        referred: Display,
        build: dilate_erode,
        doc: "Grows bright areas by a distance (or shrinks them when negative) with a feathered edge.",
    },
    Median {
        name: "median",
        title: "Median despeckle",
        family: Detail,
        referred: Display,
        build: median,
        doc: "Replaces pixels by the median of their neighbourhood, or only outliers above a threshold.",
    },
    UnsharpMask {
        name: "unsharp_mask",
        title: "Unsharp mask",
        family: Detail,
        referred: Display,
        build: unsharp_mask,
        doc: "Sharpens by adding back the difference from a Gaussian blur of a chosen radius above a threshold.",
    },
    Bilateral {
        name: "bilateral",
        title: "Bilateral smooth",
        family: Detail,
        referred: Display,
        build: bilateral,
        doc: "Smooths while keeping edges of colour, depth and normals, for skin smoothing and denoising.",
    },
    EdgeDetect {
        name: "edge_detect",
        title: "Edge detect",
        family: Detail,
        referred: Display,
        build: edge_detect,
        doc: "Outputs edge strength from colour, luminance, depth or normals with Sobel, Prewitt, Kirsch or Laplacian kernels.",
    },
    Streaks {
        name: "streaks",
        title: "Glare streaks",
        family: Glare,
        referred: Scene,
        build: streaks,
        doc: "Draws star streaks from bright light in any number of directions, with fade and colour shift.",
    },
    FogGlow {
        name: "fog_glow",
        title: "Fog glow",
        family: Glare,
        referred: Scene,
        build: fog_glow,
        doc: "Surrounds bright light with a wide, power-law glow like scatter in fog or the eye.",
    },
    LensFlare {
        name: "lens_flare",
        title: "Lens flare",
        family: Glare,
        referred: Scene,
        build: lens_flare,
        doc: "Adds ghosts with chromatic shift, a halo and an aperture starburst from bright light.",
    },
    Halation {
        name: "halation",
        title: "Halation",
        family: Film,
        referred: Scene,
        build: halation,
        doc: "Glows red-orange around highlights like light scattered back from a film base.",
    },
    GateWeave {
        name: "gate_weave",
        title: "Gate weave",
        family: Film,
        referred: Scene,
        build: gate_weave,
        doc: "Shakes and rotates the frame and flickers its exposure like film running through a projector gate.",
    },
    Cdl {
        name: "cdl",
        title: "ASC CDL",
        family: Color,
        referred: Scene,
        build: cdl,
        doc: "Applies an ASC colour decision list: slope, offset, power and saturation, linear or in ACEScct.",
    },
    WhiteBalance {
        name: "white_balance",
        title: "White balance",
        family: Color,
        referred: Scene,
        build: white_balance,
        doc: "Neutralizes light of a colour temperature and tint with a Bradford chromatic adaptation to D65.",
    },
    Trails {
        name: "trails",
        title: "Trails",
        family: Time,
        referred: Scene,
        build: trails,
        doc: "Leaves echoes of earlier frames that fade by a persistence per frame.",
    },
    FrameBlend {
        name: "frame_blend",
        title: "Frame blend",
        family: Time,
        referred: Scene,
        build: frame_blend,
        doc: "Blends each frame with the previous one like a film shutter open across both.",
    },
    Datamosh {
        name: "datamosh",
        title: "Datamosh",
        family: Time,
        referred: Display,
        build: datamosh,
        doc: "Drags earlier frames along the motion in blocks, refreshing some, like corrupted video.",
    },
}

impl BuiltinEffect {
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|builtin| builtin.name() == name)
    }

    pub fn is_display_referred(self) -> bool {
        self.referred() == Referred::Display
    }
}

fn effect(builtin: BuiltinEffect) -> Effect {
    Effect::new(builtin.name()).input(Composite::IMAGE)
}

fn image() -> EffectInput {
    EffectInput::Input(Composite::IMAGE.to_owned())
}

fn socket(name: &str) -> EffectInput {
    EffectInput::Input(name.to_owned())
}

fn single(
    builtin: BuiltinEffect,
    entry: &str,
    extend: impl FnOnce(EffectStep) -> EffectStep,
) -> Effect {
    let step = pass(builtin.name(), entry, image(), EffectOutput::Image);
    effect(builtin).step(extend(step))
}

fn target(format: EffectFormat, divisor: u32) -> EffectTarget {
    EffectTarget::new(
        format,
        match divisor {
            1 => EffectSize::Full,
            divisor => EffectSize::Divided(divisor),
        },
    )
}

fn read(name: &str) -> EffectInput {
    EffectInput::Target(name.to_owned())
}

fn write(name: &str) -> EffectOutput {
    EffectOutput::Target(name.to_owned())
}

fn pass(program: &str, entry: &str, source: EffectInput, output: EffectOutput) -> EffectStep {
    EffectStep::new(EffectCode::builtin(program, entry))
        .label(entry)
        .read("source", source)
        .write("target", output)
        .uniform("size", EffectUniform::Size)
}

fn step(program: &str, entry: &str, output: EffectOutput) -> EffectStep {
    EffectStep::new(EffectCode::builtin(program, entry))
        .label(entry)
        .write("target", output)
        .uniform("size", EffectUniform::Size)
}
