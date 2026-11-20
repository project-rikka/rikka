use std::path::Path;
use std::sync::Arc;

use rikka_model::Instance;
use rikka_model::glam::{Mat4, Vec3};
use rikka_model::humanoid::Anatomy;
use rikka_model::mmd::Vmd;
use rikka_model_gpu::{GpuModel, HeadRest};
use rikka_renderer::Renderer;
use rikka_renderer_3d::Result;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::{
    Background, Camera, Figure, Frame, Head, Look, Operation, Viewport, ViewportConfig,
};

use crate::allocations::{Phase, counted};
use crate::studio::EXTENT;

const FRAME_RATE: f32 = 60.0;
const SPACING: f32 = 0.9;
const WARMUP_FRAMES: usize = 16;

pub(crate) struct Cast {
    viewport: Viewport,
    members: Vec<Member>,
    look: Look,
    allocated: u64,
}

struct Member {
    gpu: GpuModel,
    instance: Instance,
    head: Option<Head>,
}

impl Cast {
    pub(crate) fn new(renderer: &mut Renderer) -> Result<Cast> {
        let root = Path::new("crates/model");
        let miku = root.join("rikka-mmd/assets/mmd");
        let members = [
            (
                load(&miku.join("miku.pmx"), Some(&miku.join("motion.vmd")))?,
                -SPACING,
            ),
            (
                load(&root.join("rikka-vrm/assets/avatar.vrm"), None)?,
                SPACING,
            ),
        ]
        .into_iter()
        .map(|(model, x)| member(renderer, model, x))
        .collect::<Result<Vec<_>>>()?;
        let viewport = renderer
            .three()
            .create_viewport(ViewportConfig::new(EXTENT.0, EXTENT.1))?;
        let mut look = Look::default();
        for node in &mut look.composite.nodes {
            if let Operation::DepthOfField(lens) = &mut node.operation {
                lens.aperture = f32::INFINITY;
            }
        }
        let mut cast = Cast {
            viewport,
            members,
            look,
            allocated: 0,
        };
        for _ in 0..WARMUP_FRAMES {
            cast.draw(renderer, 0, Phase::Warming)?;
        }
        Ok(cast)
    }

    pub(crate) fn allocated(&self) -> u64 {
        self.allocated
    }

    pub(crate) fn draw(&mut self, renderer: &mut Renderer, at: usize, phase: Phase) -> Result<f32> {
        let time = at as f32 / FRAME_RATE;
        let camera = Camera::look_at(
            Vec3::new(0.0, 1.3, 2.4),
            Vec3::new(0.0, 1.1, 0.0),
            Vec3::Y,
            0.8,
            EXTENT.0 as f32 / EXTENT.1 as f32,
            0.05,
        );
        let mut frame = Frame::new(camera);
        frame
            .background(Background::Sky)
            .time(time)
            .look(self.look.clone());
        for member in &self.members {
            let figure = frame.figure(Figure {
                head: member.head,
                ..Figure::default()
            });
            member
                .gpu
                .drawing(&member.instance)
                .figure(figure)
                .draw_into(&mut frame);
        }
        let (rendered, allocated) = counted(phase, || {
            renderer.three().render(&mut self.viewport, &frame)
        });
        rendered?;
        self.allocated = allocated;
        Ok((EXTENT.0 * EXTENT.1) as f32)
    }
}

fn load(path: &Path, motion: Option<&Path>) -> Result<rikka_model::Model> {
    let failed = |error: rikka_model::Error| {
        rikka_renderer::Error::Invalid(format!("{}: {error}", path.display()))
    };
    let mut model = rikka_model::import(path).map_err(failed)?;
    if let Some(motion) = motion {
        let motion = rikka_model::import(motion).map_err(failed)?;
        if let Some(vmd) = motion.extensions.get::<Vmd>() {
            let animation = vmd.animation(&model);
            model.animations.push(animation);
        }
    }
    Ok(model)
}

fn member(renderer: &mut Renderer, model: rikka_model::Model, x: f32) -> Result<Member> {
    let model = Arc::new(model);
    let gpu = GpuModel::upload(renderer, Arc::clone(&model))
        .map_err(|error| rikka_renderer::Error::Invalid(format!("upload: {error}")))?;
    let mut instance = gpu.instance();
    instance.set_transform(Mat4::from_translation(Vec3::X * x));
    if let Some(animation) = model.animations.first()
        && let Some(id) = model.find_animation(&animation.name)
    {
        instance.play(id);
        instance.advance(3.0);
    }
    instance.refresh();
    let head = HeadRest::new(&model, &Anatomy::recognize(&model)).map(|head| head.place(&instance));
    Ok(Member {
        gpu,
        instance,
        head,
    })
}
