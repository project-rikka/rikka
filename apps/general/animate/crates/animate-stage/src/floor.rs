use animate_look::world::{Floor, FloorKind};
use rikka_model::glam::{Vec2, Vec3, Vec4};
use rikka_renderer::Renderer;
use rikka_renderer_3d::{
    Draw, FloorParams, FloorPattern, Frame, Indices, MaterialId, MeshData, MeshId, Three,
    WaterParams,
};

const HALF: f32 = 200.0;
const OBJECT: u32 = u32::MAX >> 2;
const SHALLOW_LIFT: f32 = 5.0;

#[derive(Debug, Default)]
pub(super) struct Floors {
    state: State,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    Unbuilt,
    Built(Built),
    Failed,
}

#[derive(Debug)]
struct Built {
    plane: MeshId,
    floor: MaterialId,
    shadow: MaterialId,
    water: MaterialId,
}

impl Floors {
    pub(super) fn draw(
        &mut self,
        renderer: &mut Renderer,
        frame: &mut Frame,
        stage: &Floor,
    ) -> rikka_renderer_3d::Result<()> {
        if stage.kind == FloorKind::None {
            return Ok(());
        }
        let Some(built) = self.built(renderer)? else {
            return Ok(());
        };
        let (material, values) = match stage.kind {
            FloorKind::Water => (built.water, water(stage).values()),
            FloorKind::Shadow => (built.shadow, shadow(stage).values()),
            _ => (built.floor, floor(stage).values()),
        };
        renderer.three().set_material_values(material, &values)?;
        frame.draw(
            Draw::new(built.plane, material)
                .object_id(OBJECT)
                .pickable(false),
        );
        Ok(())
    }

    fn built(&mut self, renderer: &mut Renderer) -> rikka_renderer_3d::Result<Option<&Built>> {
        if matches!(self.state, State::Unbuilt) {
            self.state = match build(renderer) {
                Ok(built) => State::Built(built),
                Err(error) => {
                    self.state = State::Failed;
                    return Err(error);
                }
            };
        }
        Ok(match &self.state {
            State::Built(built) => Some(built),
            State::Unbuilt | State::Failed => None,
        })
    }
}

fn floor(stage: &Floor) -> FloorParams {
    FloorParams {
        color: stage.color,
        roughness: stage.roughness,
        reflection: stage.reflection,
        pattern: match stage.kind {
            FloorKind::Tiles => FloorPattern::Tiles,
            _ => FloorPattern::Plain,
        },
        scale: stage.tile_scale,
        ..FloorParams::default()
    }
}

fn shadow(stage: &Floor) -> FloorParams {
    FloorParams {
        color: Vec3::ZERO,
        reflection: 0.0,
        pattern: FloorPattern::Shadow,
        shadow: stage.shadow,
        ..FloorParams::default()
    }
}

fn water(stage: &Floor) -> WaterParams {
    WaterParams {
        deep: stage.water_color,
        shallow: (stage.water_color * SHALLOW_LIFT).min(Vec3::ONE),
        height: stage.wave_height,
        speed: stage.wave_speed,
        ..WaterParams::default()
    }
}

fn build(renderer: &mut Renderer) -> rikka_renderer_3d::Result<Built> {
    let positions = [
        Vec3::new(-HALF, 0.0, -HALF),
        Vec3::new(HALF, 0.0, -HALF),
        Vec3::new(HALF, 0.0, HALF),
        Vec3::new(-HALF, 0.0, HALF),
    ];
    let uvs = positions.map(|position| Vec2::new(position.x, position.z));
    let plane = renderer.three().create_mesh(&MeshData {
        positions: &positions,
        normals: &[Vec3::Y; 4],
        tangents: &[Vec4::new(1.0, 0.0, 0.0, 1.0); 4],
        uvs: &uvs,
        indices: Some(Indices::U32(&[0, 2, 1, 0, 3, 2])),
        ..MeshData::default()
    })?;
    let floor = renderer
        .three()
        .create_material(FloorParams::default().material()?)?;
    let shadow = renderer
        .three()
        .create_material(shadow(&Floor::default()).material()?)?;
    let water = renderer
        .three()
        .create_material(WaterParams::default().material()?)?;
    Ok(Built {
        plane,
        floor,
        shadow,
        water,
    })
}
