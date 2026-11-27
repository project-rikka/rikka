mod assets;
mod materials;
mod passes;
pub(crate) mod wake;

use animate_cast::{FigureId, PropId};
use animate_look::{Evaluated, GraphRef, Linked, Look, Name, Plans, Resolver, shared_library};
use animate_motion::MotionBake;
use animate_shader::{Listener, SharedCompiler, SharedShaders};
use rikka_gpu::TextureId;
use rikka_model::glam::Vec3;
use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_model::{Instance, MaterialId};
use rikka_model_gpu::GpuModel;
use rikka_renderer::Renderer;
use rikka_renderer_3d::{
    BuiltinEffect, Camera, Composite, EffectId, FigureId as DrawnFigure, Frame, Grid, Head,
    LightProfileId, PanoramaSun, ParameterSpec, TextureBinding, TextureKind,
};
use rikka_shader_compile::{Resolve, composite};
use rikka_shader_graph::{PixelStep, SocketRef, SocketValue};
use rustc_hash::FxHashMap;

use crate::floor::Floors;
use crate::marks::EmitterMark;
use assets::Assets;
use materials::{Dressed, Sources};
use passes::Passes;
use wake::Wake;

pub use assets::is_color_lookup;

const GRID_COLOR: Vec3 = Vec3::new(0.9, 0.92, 0.95);
const GRID_OPACITY: f32 = 0.25;
const GRID_SPACING: f32 = 5.0;
const GRID_REACH: f32 = 50.0;
const NEAREST_FOCUS: f32 = 0.1;

pub(super) struct Player<'a> {
    pub(super) id: FigureId,
    pub(super) name: &'a str,
    pub(super) look_name: Name,
    pub(super) head: Option<Head>,
    pub(super) instance: &'a Instance,
    pub(super) focused: bool,
    pub(super) channel: u32,
}

pub(super) struct Moment<'a, 'p> {
    pub(super) camera: &'a Camera,
    pub(super) time: f32,
    pub(super) players: &'a [Player<'p>],
}

pub(super) struct Staged {
    pub(super) figures: FxHashMap<FigureId, DrawnFigure>,
}

pub(super) struct Dressees<'a, 'p> {
    pub(super) props: &'a [(PropId, Name)],
    pub(super) gpus: &'p mut FxHashMap<PropId, GpuModel>,
}

pub struct Dressing {
    shaders: SharedShaders,
    passes: Passes,
    assets: Assets,
    floors: Floors,
    plans: Plans,
    dressed: FxHashMap<FigureId, Dressed>,
    dressed_props: FxHashMap<PropId, Dressed>,
    marks: Vec<EmitterMark>,
    wake: Wake,
    listener: Option<Listener>,
    failures: Vec<String>,
    reported: Option<String>,
}

impl std::fmt::Debug for Dressing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dressing")
            .field("dressed", &self.dressed.len())
            .field("wake", &self.wake)
            .finish_non_exhaustive()
    }
}

impl Dressing {
    pub fn new(shaders: SharedShaders, compiler: SharedCompiler) -> Self {
        Self {
            shaders,
            passes: Passes::new(compiler),
            assets: Assets::default(),
            floors: Floors::default(),
            plans: Plans::default(),
            dressed: FxHashMap::default(),
            dressed_props: FxHashMap::default(),
            marks: Vec::new(),
            wake: Wake::default(),
            listener: None,
            failures: Vec::new(),
            reported: None,
        }
    }

    pub(super) fn replica(&self) -> Self {
        Self::new(self.shaders.clone(), self.passes.compiler().clone())
    }

    pub(super) fn notify_with(&mut self, notify: rikka_physics::ProgressCallback) {
        self.wake.notify_with(notify);
        let listener = self.wake.listener();
        self.shaders.lock().watch(&listener);
        self.listener = Some(listener);
    }

    pub(super) fn take_roused(&self) -> bool {
        self.wake.take()
    }

    pub(super) fn forget(&mut self, figure: FigureId) {
        self.dressed.remove(&figure);
    }

    pub(super) fn forget_prop(&mut self, prop: PropId) {
        self.dressed_props.remove(&prop);
    }

    pub(super) fn is_settled(&self) -> bool {
        !self.wake.is_waiting()
    }

    pub(super) fn await_bake(&self, motion_bake: &MotionBake, estimated: bool) {
        if estimated {
            self.wake.wait();
        }
        if let Some(listener) = &self.listener {
            motion_bake.watch(listener);
        }
    }

    pub(super) fn marks(&self) -> &[EmitterMark] {
        &self.marks
    }

    pub(super) fn take_failure(&mut self) -> Option<String> {
        (!self.failures.is_empty()).then(|| std::mem::take(&mut self.failures).join("; "))
    }

    pub(super) fn stage(
        &mut self,
        renderer: &mut Renderer,
        frame: &mut Frame,
        look: &Look,
        moment: Moment<'_, '_>,
        gpus: &mut FxHashMap<FigureId, GpuModel>,
    ) -> Staged {
        let mut none = FxHashMap::default();
        let dressees = Dressees {
            props: &[],
            gpus: &mut none,
        };
        self.stage_with(renderer, frame, look, moment, gpus, dressees)
    }

    pub(super) fn stage_with(
        &mut self,
        renderer: &mut Renderer,
        frame: &mut Frame,
        look: &Look,
        moment: Moment<'_, '_>,
        gpus: &mut FxHashMap<FigureId, GpuModel>,
        dressees: Dressees<'_, '_>,
    ) -> Staged {
        let Moment {
            camera,
            time,
            players,
        } = moment;
        self.wake.begin_frame();
        self.passes.begin();
        self.marks.clear();
        let library = shared_library();
        let mut evaluated = match self
            .plans
            .evaluate(look, library, time * FRAMES_PER_SECOND, time)
        {
            Ok(evaluated) => {
                self.reported = None;
                evaluated
            }
            Err(error) => {
                let reason = error.to_string();
                if self.reported.as_ref() != Some(&reason) {
                    self.failures.push(reason.clone());
                    self.reported = Some(reason);
                }
                return Staged {
                    figures: players
                        .iter()
                        .map(|player| (player.id, frame.figure(Default::default())))
                        .collect(),
                };
            }
        };
        let eye = camera.position();
        let focus = focus_point(players);
        if let Some(focus) = focus {
            evaluated.autofocus(focus.distance(eye).max(NEAREST_FOCUS));
        }
        let world = &evaluated.world;
        let figures: FxHashMap<FigureId, DrawnFigure> = {
            let mut resolving = Resolving {
                renderer: &mut *renderer,
                assets: &mut self.assets,
                passes: &mut self.passes,
                wake: &self.wake,
                failures: &mut self.failures,
                evaluated: &evaluated,
                gpus,
                shaders: &self.shaders,
            };
            let mut built = world.look(&mut resolving);
            self.marks.extend(
                world
                    .emitters
                    .iter()
                    .zip(&built.emitters)
                    .map(|(look, emitter)| EmitterMark {
                        node: look.node.clone(),
                        shape: emitter.shape,
                        transform: emitter.track.transform_at(time),
                    }),
            );
            built.composite =
                match composite(&evaluated.composite, &evaluated.records, &mut resolving) {
                    Ok(composite) => composite,
                    Err(error) => {
                        resolving.failures.push(format!("composite: {error}"));
                        Composite::new()
                    }
                };
            frame
                .look(built)
                .ambient(world.ambient())
                .background(world.background());
            let linked: Vec<Linked<'_>> = players
                .iter()
                .map(|player| Linked {
                    name: player.name,
                    channel: player.channel,
                })
                .collect();
            for light in world.lights(focus.unwrap_or(Vec3::ZERO), eye, &linked, &mut resolving) {
                match light {
                    Ok(light) => {
                        frame.light(light);
                    }
                    Err(error) => resolving.failures.push(error.to_string()),
                }
            }
            players
                .iter()
                .map(|player| {
                    let drawn = world.drawn_figure(player.name, player.head, &mut resolving);
                    (player.id, frame.figure(drawn))
                })
                .collect()
        };
        if let Some(floor) = world.floor()
            && let Err(error) = self.floors.draw(renderer, frame, floor)
        {
            self.failures.push(format!("floor: {error}"));
        }
        frame.grid(Grid {
            enabled: world.grid(),
            color: GRID_COLOR,
            opacity: GRID_OPACITY,
            spacing: GRID_SPACING,
            reach: GRID_REACH,
            vertical_axis: false,
        });
        let mut sources = Sources {
            shaders: &self.shaders,
            assets: &mut self.assets,
            wake: &self.wake,
            failures: &mut self.failures,
        };
        for player in players {
            let Some(gpu) = gpus.get_mut(&player.id) else {
                continue;
            };
            let dressed = self.dressed.entry(player.id).or_default();
            dressed.dress(renderer, gpu, player.look_name, &evaluated, &mut sources);
        }
        for (id, look_name) in dressees.props {
            let Some(gpu) = dressees.gpus.get_mut(id) else {
                continue;
            };
            let dressed = self.dressed_props.entry(*id).or_default();
            dressed.dress(renderer, gpu, *look_name, &evaluated, &mut sources);
        }
        self.passes.sweep(renderer);
        Staged { figures }
    }
}

struct Resolving<'a, 'e> {
    renderer: &'a mut Renderer,
    assets: &'a mut Assets,
    passes: &'a mut Passes,
    wake: &'a Wake,
    failures: &'a mut Vec<String>,
    evaluated: &'a Evaluated<'e>,
    gpus: &'a FxHashMap<FigureId, GpuModel>,
    shaders: &'a SharedShaders,
}

impl Resolver for Resolving<'_, '_> {
    fn texture(&mut self, path: &str, kind: TextureKind) -> Option<TextureBinding> {
        self.assets
            .texture(self.renderer, path, kind, self.wake, self.failures)
    }

    fn panorama(&mut self, path: &str) -> Option<TextureId> {
        self.assets
            .panorama(self.renderer, path, self.wake, self.failures)
    }

    fn panorama_sun(&mut self, path: &str, threshold: f32) -> Option<PanoramaSun> {
        self.assets
            .panorama_sun(self.renderer, path, threshold, self.wake, self.failures)
    }

    fn profile(&mut self, path: &str) -> Option<LightProfileId> {
        self.assets
            .profile(self.renderer, path, self.wake, self.failures)
    }

    fn parameter(&mut self, name: &str) -> Option<ParameterSpec> {
        let imported = self.gpus.values().find_map(|gpu| {
            (0..gpu.model().materials.len()).find_map(|index| {
                let base = gpu.base(MaterialId(index as u32))?;
                base.template().schema().parameter(name).cloned()
            })
        });
        imported.or_else(|| {
            let shaders = self.shaders.lock();
            let keys: Vec<String> = shaders.keys().map(str::to_owned).collect();
            keys.iter().find_map(|key| {
                shaders
                    .material(key)?
                    .template()
                    .schema()
                    .parameter(name)
                    .cloned()
            })
        })
    }
}

impl Resolve for Resolving<'_, '_> {
    fn pixel(&mut self, step: &PixelStep) -> Option<EffectId> {
        self.passes
            .pixel(self.renderer, step, self.wake, self.failures)
    }

    fn builtin(&mut self, effect: BuiltinEffect) -> Option<EffectId> {
        self.passes.builtin(self.renderer, effect, self.failures)
    }

    fn socket(&mut self, at: &SocketRef) -> Option<SocketValue> {
        self.evaluated.socket(&GraphRef::Composite, at)
    }

    fn texture(&mut self, path: &str) -> Option<TextureBinding> {
        self.assets.texture(
            self.renderer,
            path,
            TextureKind::Colour,
            self.wake,
            self.failures,
        )
    }
}

fn focus_point(players: &[Player<'_>]) -> Option<Vec3> {
    let player = players
        .iter()
        .find(|player| player.focused)
        .or_else(|| players.first())?;
    Some(player.head.map_or_else(
        || {
            let bounds = player.instance.bounds();
            (bounds.min + bounds.max) * 0.5
        },
        |head| head.center,
    ))
}
