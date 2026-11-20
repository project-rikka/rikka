use std::time::{Duration, Instant};

use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    Background, Camera, Cull, Draw, Frame, Light, MaterialDesc, MaterialState, MaterialTemplate,
    ShaderCompiler, ShadowQuality, TemplateReadiness, Three, ToneMapping, Viewport, ViewportConfig,
};
use rikka_shader_compile::Material;
use rikka_shader_graph::{
    Arg, Builder, Curve, DirectionSpace, Graph, GraphKind, Input, Interpolation, Library, NodeKind,
    Op,
};
use rikka_shader_programs::Variant;
use rikka_slang::{Compiler, Optimization, Source};

const SIZE: u32 = 64;

const SETTLE_LIMIT: Duration = Duration::from_secs(300);

struct Sample {
    name: &'static str,
    build: fn(&Library, bool) -> Graph,
    outlined: bool,
}

const SAMPLES: [Sample; 6] = [
    Sample {
        name: "emission",
        build: emission,
        outlined: false,
    },
    Sample {
        name: "principled",
        build: principled,
        outlined: false,
    },
    Sample {
        name: "cel-outline",
        build: cel_outline,
        outlined: true,
    },
    Sample {
        name: "two-round",
        build: two_round,
        outlined: false,
    },
    Sample {
        name: "closures-2",
        build: closures_two,
        outlined: false,
    },
    Sample {
        name: "closures-4",
        build: closures_four,
        outlined: false,
    },
];

fn tint(builder: &mut Builder<'_>, edited: bool, color: [f32; 3]) -> Arg {
    match edited {
        true => builder.multiply(Arg::from(color), Arg::from(0.9)).into(),
        false => color.into(),
    }
}

fn emission(library: &Library, edited: bool) -> Graph {
    let mut builder = Builder::new(GraphKind::Material, library);
    let color = tint(&mut builder, edited, [1.0, 0.2, 0.1]);
    let shader = builder.shader(NodeKind::Emission, [("color", color)]);
    builder.output([("surface", shader.into())]);
    builder.finish().expect("a valid graph")
}

fn principled(library: &Library, edited: bool) -> Graph {
    let mut builder = Builder::new(GraphKind::Material, library);
    let color = tint(&mut builder, edited, [0.8, 0.3, 0.2]);
    let surface = builder.external(
        "bsdf.principled",
        [("base_color", color), ("roughness", 0.4.into())],
    );
    let shader = builder.out(surface, "shader");
    builder.output([("surface", shader.into())]);
    builder.finish().expect("a valid graph")
}

fn heavy(builder: &mut Builder<'_>, edited: bool, index: usize) -> Arg {
    let color = tint(builder, edited, [0.8, 0.3 + 0.1 * index as f32, 0.2]);
    let closure = match index {
        0 => builder.external("bsdf.skin", [("subsurface_color", color)]),
        _ => builder.external(
            "bsdf.principled",
            [
                ("base_color", color),
                ("coat_weight", (0.2 * index as f32).into()),
            ],
        ),
    };
    builder.out(closure, "shader").into()
}

fn mixed(builder: &mut Builder<'_>, a: Arg, b: Arg) -> Arg {
    builder
        .shader(
            NodeKind::MixShader,
            [("a", a), ("b", b), ("factor", 0.5.into())],
        )
        .into()
}

fn closures_two(library: &Library, edited: bool) -> Graph {
    let mut builder = Builder::new(GraphKind::Material, library);
    let [a, b] = [0, 1].map(|index| heavy(&mut builder, edited, index));
    let shader = mixed(&mut builder, a, b);
    builder.output([("surface", shader)]);
    builder.finish().expect("a valid graph")
}

fn closures_four(library: &Library, edited: bool) -> Graph {
    let mut builder = Builder::new(GraphKind::Material, library);
    let [a, b, c, d] = [0, 1, 2, 3].map(|index| heavy(&mut builder, edited, index));
    let first = mixed(&mut builder, a, b);
    let second = mixed(&mut builder, c, d);
    let shader = mixed(&mut builder, first, second);
    builder.output([("surface", shader)]);
    builder.finish().expect("a valid graph")
}

fn cel_outline(library: &Library, edited: bool) -> Graph {
    let mut builder = Builder::new(GraphKind::Material, library);
    let normal = builder.input(Input::Normal(DirectionSpace::World));
    let toward = builder.input(Input::LightDirection);
    let facing = builder.dot(normal, toward);
    let band = builder.external(
        "npr.toon_step",
        [
            ("value", facing.into()),
            ("edge", 0.0.into()),
            ("feather", 0.0.into()),
        ],
    );
    let band = builder.out(band, "value");
    let color = builder.input(Input::LightColor);
    let lit = builder.multiply(color, band);
    let paint = tint(&mut builder, edited, [0.5, 0.25, 0.125]);
    let tinted = builder.multiply(lit, paint);
    let shader = builder.shader(NodeKind::Lighting, [("light", tinted.into())]);
    let line = builder.shader(NodeKind::Emission, [("color", [0.1, 0.0, 0.0].into())]);
    builder.output([
        ("surface", shader.into()),
        ("outline", line.into()),
        ("outline_width", 0.1.into()),
    ]);
    builder.finish().expect("a valid graph")
}

fn two_round(library: &Library, edited: bool) -> Graph {
    let mut builder = Builder::new(GraphKind::Material, library);
    let color = tint(&mut builder, edited, [1.0, 1.0, 1.0]);
    let base = builder.external("bsdf.principled", [("base_color", color)]);
    let base = builder.out(base, "shader");
    let seen = builder.with(NodeKind::ShaderToRgb, [("shader", base.into())]);
    let seen = builder.out(seen, "color");
    let brightness = builder.op(Op::Luminance, [seen.into()]);
    let banded = builder.curve(
        Curve {
            points: vec![[0.0, 0.0], [0.3, 1.0]],
            interpolation: Interpolation::Constant,
        },
        brightness,
    );
    let light = builder.input(Input::LightColor);
    let relit = builder.multiply(light, banded);
    let shader = builder.shader(NodeKind::Lighting, [("light", relit.into())]);
    builder.output([("surface", shader.into())]);
    builder.finish().expect("a valid graph")
}

fn seconds(duration: Duration) -> String {
    format!("{:.2}s", duration.as_secs_f64())
}

fn timed<T>(run: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = run();
    (value, start.elapsed())
}

struct Cost {
    time: Duration,
    growth: u64,
}

impl std::fmt::Display for Cost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{:.2}s/{:.2}G",
            self.time.as_secs_f64(),
            self.growth as f64 / (1024.0 * 1024.0)
        )
    }
}

fn resident_peak() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").expect("procfs");
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
        .expect("VmHWM in kB")
}

fn resident() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").expect("procfs");
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
        .expect("VmRSS in kB")
}

fn measured<T>(run: impl FnOnce() -> T) -> (T, Cost) {
    std::fs::write("/proc/self/clear_refs", "5").expect("peak resident reset");
    let before = resident();
    let (value, time) = timed(run);
    let growth = resident_peak().saturating_sub(before);
    (value, Cost { time, growth })
}

fn slang_compiler(optimization: Optimization) -> Compiler {
    Compiler::builder()
        .modules(shaders_3d::SLANG_LIBRARY)
        .optimization(optimization)
        .build()
        .expect("libslang loads")
}

fn breakdown(library: &Library, samples: &[&Sample]) {
    let (high, opened) = timed(|| slang_compiler(Optimization::High));
    let none = slang_compiler(Optimization::None);
    let probe = slang_compiler(Optimization::None);
    println!("compiler sessions opened in {}", seconds(opened));
    for sample in samples {
        let source = (sample.build)(library, false)
            .material(library, sample.name)
            .expect("the graph generates")
            .source;
        let wrapped = rikka_shader_programs::source(&source);
        let reflect = |compiler: &Compiler| {
            timed(|| {
                compiler
                    .reflect(&Source::new(sample.name, &wrapped).layout_of("RikkaMaterialBlock"))
                    .expect("the graph links")
            })
            .1
        };
        let first = reflect(&high);
        let warm = reflect(&high);
        println!(
            "{}: {} source lines, front end {} cold / {} warm",
            sample.name,
            source.lines().count(),
            seconds(first),
            seconds(warm)
        );
        for (label, compiler) in [("-O2", &high), ("-O0", &none)] {
            let mut total = Duration::ZERO;
            let mut row = Vec::new();
            for variant in Variant::ALL {
                let (_, cost) = measured(|| {
                    compiler
                        .compile(&Source::new(sample.name, &wrapped).entry(variant.entry()))
                        .expect("the variant compiles")
                });
                total += cost.time;
                row.push(format!(
                    "{}={cost}",
                    variant.entry().trim_start_matches("rikka_entry_"),
                ));
            }
            println!(
                "  {label} per variant (serial sum {}): {}",
                seconds(total),
                row.join(" ")
            );
        }
        let (_, probed) = measured(|| {
            none.compile(&Source::new(sample.name, &wrapped).entry("rikka_entry_probe"))
                .expect("the probe compiles")
        });
        let (_, all) = measured(|| {
            rikka_shader_programs::compile_template(&high, Some(&none), sample.name, &wrapped)
                .expect("the template compiles")
        });
        let (_, drafted) = measured(|| {
            rikka_shader_programs::draft_template(&none, Some(&probe), sample.name, &wrapped)
                .expect("the draft compiles")
        });
        let (_, unprobed) = measured(|| {
            rikka_shader_programs::draft_template(&none, None, sample.name, &wrapped)
                .expect("the draft compiles")
        });
        let (_, forward) = measured(|| {
            none.compile(
                &Source::new(sample.name, &wrapped)
                    .entry(Variant::Forward.entry())
                    .flatten(),
            )
            .expect("the forward variant compiles")
        });
        println!(
            "  probe {probed} ; compile_template -O2 with probe {all} ; draft_template with \
             probe {drafted} / without {unprobed} ; flattened forward alone {forward}",
        );
    }
}

fn frame(mesh: rikka_renderer_3d::MeshId, materials: &[rikka_renderer_3d::MaterialId]) -> Frame {
    let mut frame = Frame::new(Camera::look_at(
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    ));
    frame
        .background(Background::Color(Vec3::ZERO))
        .ambient(Vec3::splat(0.2))
        .tone_mapping(ToneMapping::Clamp)
        .light(
            Light::directional(Vec3::new(-1.0, -0.3, -1.0))
                .intensity(2.0)
                .shadow(ShadowQuality::Medium),
        );
    for material in materials {
        frame.draw(Draw::new(mesh, *material));
    }
    frame
}

struct Settled {
    drawn: Duration,
    built: Duration,
    frames: usize,
}

fn settle(
    renderer: &mut rikka_renderer::Renderer,
    viewport: &mut Viewport,
    frame: &Frame,
    template: &MaterialTemplate,
    start: Instant,
    first_draws: bool,
) -> Settled {
    let mut drawn = None;
    let mut frames = 0;
    loop {
        let readiness = template.readiness();
        let drawable = first_draws || template.is_ready();
        renderer.three().render(viewport, frame).expect("a frame");
        renderer.gpu().device().wait_idle();
        frames += 1;
        let at = start.elapsed();
        if drawable {
            drawn.get_or_insert(at);
        }
        match readiness {
            TemplateReadiness::Built => {
                return Settled {
                    drawn: drawn.unwrap_or(at),
                    built: at,
                    frames,
                };
            }
            TemplateReadiness::Failed(error) => panic!("the optimized build failed: {error}"),
            TemplateReadiness::Pending | TemplateReadiness::Drawable => {}
        }
        assert!(at < SETTLE_LIMIT, "the template never finished building");
    }
}

fn end_to_end(library: &Library, samples: &[&Sample]) {
    let mut renderer = rikka_renderer::Renderer::new(
        &rikka_gpu::Gpu::builder()
            .app_name("material-bench")
            .with_3d()
            .build()
            .expect("Vulkan 1.3 device"),
    )
    .expect("Vulkan 1.3 device");
    let (compiler, opened) = timed(|| ShaderCompiler::new().expect("libslang loads"));
    println!("ShaderCompiler::new {}", seconds(opened));
    let mesh = rikka_test_renderer_3d::sphere(&mut renderer, 1.2, 24).expect("a sphere");
    let mut viewport = renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))
        .expect("a viewport");
    for sample in samples {
        let graph = (sample.build)(library, false);
        let start = Instant::now();
        let mut material =
            Material::compile(&graph, library, &compiler, sample.name).expect("compiles");
        let compiled = start.elapsed();
        let template = material.template().clone();
        let mut ids = vec![
            renderer
                .three()
                .create_material(MaterialDesc::new(&template))
                .expect("a material"),
        ];
        if sample.outlined {
            ids.push(
                renderer
                    .three()
                    .create_material(
                        MaterialDesc::new(&template)
                            .state(MaterialState::opaque().cull(Cull::Front))
                            .hull(),
                    )
                    .expect("a hull material"),
            );
        }
        let frame = frame(mesh, &ids);
        let settled = settle(&mut renderer, &mut viewport, &frame, &template, start, true);
        println!(
            "{} new: draft {} drawable {} built {}",
            sample.name,
            seconds(compiled),
            seconds(settled.drawn),
            seconds(settled.built),
        );

        let edited = (sample.build)(library, true);
        let start = Instant::now();
        material
            .reload(&edited, library, &compiler)
            .expect("reloads");
        let compiled = start.elapsed();
        let settled = settle(
            &mut renderer,
            &mut viewport,
            &frame,
            &template,
            start,
            false,
        );
        println!(
            "{} edit: draft {} drawable {} built {} ({} frames)",
            sample.name,
            seconds(compiled),
            seconds(settled.drawn),
            seconds(settled.built),
            settled.frames,
        );
        for id in ids {
            renderer
                .three()
                .destroy_material(id)
                .expect("a live material");
        }
    }
}

fn main() {
    let filters: Vec<String> = std::env::args()
        .skip(1)
        .filter(|argument| !argument.starts_with("--"))
        .collect();
    let library = rikka_shader_compile::library().expect("the node library");
    let samples: Vec<&Sample> = SAMPLES
        .iter()
        .filter(|sample| {
            filters.is_empty()
                || filters
                    .iter()
                    .any(|name| sample.name.contains(name.as_str()))
        })
        .collect();
    if std::env::args().any(|argument| argument == "--print") {
        for sample in &samples {
            let source = (sample.build)(&library, false)
                .material(&library, sample.name)
                .expect("the graph generates");
            println!("// {}\n{}", sample.name, source.source);
        }
        return;
    }
    if std::env::args().any(|argument| argument == "--breakdown") {
        breakdown(&library, &samples);
    }
    end_to_end(&library, &samples);
}
