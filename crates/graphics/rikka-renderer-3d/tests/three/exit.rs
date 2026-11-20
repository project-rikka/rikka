use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rikka_renderer::Renderer;
use rikka_renderer_3d::glam::{Vec3, Vec4};
use rikka_renderer_3d::{
    Camera, Draw, Error, Frame, MaterialDesc, MaterialState, ShaderCompiler, Three, ViewportConfig,
};
use rikka_test_renderer::TestRenderer;
use rikka_test_renderer_3d::{sphere, standard};

const CHILD: &str = "RIKKA_RENDERER_3D_EXIT_CHILD";

const TEST: &str = "three::exit::process_exits_cleanly_with_work_in_flight";

const DRAFT_EXITS: [Duration; 4] = [
    Duration::ZERO,
    Duration::from_millis(200),
    Duration::from_millis(350),
    Duration::from_millis(500),
];

const SIZE: u32 = 32;

const DIMMED: &str = r#"
import rikka;

struct Surface : ISurface<RikkaNoData> {
    [Color] [Default(1.0, 1.0, 1.0)] float3 tint;
    [Default(0.25)] float dim;

    [constref]
    RikkaSurfaceData<RikkaNoData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaNoData data;
        data.unused = 0u;
        RikkaSurfaceData<RikkaNoData> s = rikka_surface_data(pixel, data);
        s.albedo = tint * dim;
        return s;
    }
};

typealias Model = RikkaUnlitModel;
"#;

#[derive(Clone, Copy, Debug)]
enum Exit {
    WhileOpening,
    WhileDrafting(Duration),
    WhileOptimizing,
    WithLeakedRenderer,
}

impl Exit {
    fn all() -> impl Iterator<Item = Self> {
        [Self::WhileOpening]
            .into_iter()
            .chain(DRAFT_EXITS.map(Self::WhileDrafting))
            .chain([Self::WhileOptimizing, Self::WithLeakedRenderer])
    }

    fn name(self) -> String {
        match self {
            Self::WhileOpening => "opening".to_owned(),
            Self::WhileDrafting(after) => format!("drafting for {} ms", after.as_millis()),
            Self::WhileOptimizing => "optimizing".to_owned(),
            Self::WithLeakedRenderer => "leaked".to_owned(),
        }
    }

    fn named(name: &str) -> Self {
        Self::all()
            .find(|exit| exit.name() == name)
            .unwrap_or_else(|| panic!("unknown exit point {name}"))
    }

    fn spawn(self) -> Child {
        let program = std::env::current_exe().expect("test binary path");
        Command::new(program)
            .args(["--exact", TEST, "--test-threads=1"])
            .env(CHILD, self.name())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("child test process starts")
    }
}

#[test]
fn process_exits_cleanly_with_work_in_flight() {
    if let Ok(name) = std::env::var(CHILD) {
        exit_during(Exit::named(&name));
        return;
    }
    for exit in Exit::all() {
        let Output {
            status,
            stdout,
            stderr,
        } = exit
            .spawn()
            .wait_with_output()
            .expect("child test process ends");
        assert!(
            status.success(),
            "exiting while {} ended with {status}: {}{}",
            exit.name(),
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        );
    }
}

fn exit_during(exit: Exit) {
    let mut renderer = renderer();
    match exit {
        Exit::WhileOpening => drafting(None),
        Exit::WhileDrafting(after) => {
            let (opened, open) = mpsc::channel();
            drafting(Some(opened));
            open.recv().expect("the compiler opens");
            thread::sleep(after);
        }
        Exit::WhileOptimizing => {
            let template = ShaderCompiler::new()
                .expect("libslang loads")
                .template("dimmed", DIMMED)
                .expect("the template compiles");
            draw(&mut renderer, MaterialDesc::new(&template));
        }
        Exit::WithLeakedRenderer => {
            draw(
                &mut renderer,
                standard(Vec4::new(0.8, 0.2, 0.1, 1.0), MaterialState::opaque()),
            );
            std::mem::forget(renderer);
        }
    }
}

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-exit-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn drafting(opened: Option<mpsc::Sender<()>>) {
    thread::spawn(move || {
        let compiler = ShaderCompiler::new().expect("libslang loads");
        if let Some(opened) = opened {
            opened.send(()).expect("the test waits for the compiler");
        }
        loop {
            match compiler.template("dimmed", DIMMED) {
                Ok(_) => {}
                Err(Error::Slang(rikka_slang::Error::Unavailable(_))) => return,
                Err(error) => panic!("the template fails to compile: {error}"),
            }
        }
    });
}

fn draw(renderer: &mut Renderer, desc: MaterialDesc) {
    let mesh = sphere(renderer, 1.0, 8).expect("a sphere");
    let material = renderer.three().create_material(desc).expect("a material");
    let mut viewport = renderer
        .three()
        .create_viewport(ViewportConfig::new(SIZE, SIZE).samples(1))
        .expect("a viewport");
    let mut frame = Frame::new(Camera::look_at(
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    ));
    frame.draw(Draw::new(mesh, material));
    renderer
        .three()
        .render(&mut viewport, &frame)
        .expect("a 3D frame draws");
}
