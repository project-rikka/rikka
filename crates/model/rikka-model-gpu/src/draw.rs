use rikka_model_core::glam::Vec3;
use rikka_renderer_3d::{
    Camera, DEFAULT_LAYERS, DEFAULT_LIGHT_CHANNELS, Drape, Draw, FigureId, Frame,
    Light as GpuLight, Wetness, orthographic_reversed, perspective_reversed_infinite,
};

use crate::gpu_model::GpuModel;
use crate::materials;
use crate::restyle::Restyle;
use rikka_model_core::Instance;
use rikka_model_core::LightKind;
use rikka_model_core::MaterialId;
use rikka_model_core::NodeId;
use rikka_model_core::Projection;

#[derive(Debug)]
#[must_use]
pub struct ModelDrawing<'a> {
    gpu: &'a GpuModel,
    instance: &'a Instance,
    figure: Option<FigureId>,
    layers: u32,
    wet: Option<Wetness>,
    drapes: Vec<(NodeId, Drape)>,
    restyle: Option<&'a Restyle>,
    light_channels: u32,
    selected: &'a [MaterialId],
}

impl<'a> ModelDrawing<'a> {
    pub fn figure(mut self, figure: FigureId) -> Self {
        self.figure = Some(figure);
        self
    }

    pub fn layers(mut self, layers: u32) -> Self {
        self.layers = layers;
        self
    }

    pub fn wet(mut self, wetness: Wetness) -> Self {
        self.wet = Some(wetness);
        self
    }

    pub fn drape(mut self, node: NodeId, drape: Drape) -> Self {
        self.drapes.retain(|(draped, _)| *draped != node);
        self.drapes.push((node, drape));
        self
    }

    pub fn light_channels(mut self, channels: u32) -> Self {
        self.light_channels = channels;
        self
    }

    pub fn restyled(mut self, restyle: &'a Restyle) -> Self {
        self.restyle = Some(restyle);
        self
    }

    pub fn selected(mut self, materials: &'a [MaterialId]) -> Self {
        self.selected = materials;
        self
    }

    pub fn draw_into(self, frame: &mut Frame) {
        let Self {
            gpu,
            instance,
            figure,
            layers,
            wet,
            drapes,
            restyle,
            light_channels,
            selected,
        } = self;
        for (index, node) in gpu.model().nodes.iter().enumerate() {
            let Some(mesh) = node.mesh else {
                continue;
            };
            let Some(uploaded) = gpu.mesh(mesh) else {
                continue;
            };
            let id = NodeId(index as u32);
            let skin = node.skin.map(|skin| frame.skin(instance.joints(skin)));
            let weights = match instance.morph_weights(id) {
                [] => None,
                weights => Some(frame.morphs(weights)),
            };
            let drape = drapes
                .iter()
                .find(|(draped, _)| *draped == id)
                .map(|&(_, drape)| drape);
            let transform = match node.skin {
                Some(_) => instance.transform(),
                None => instance.transform() * instance.world(id),
            };
            let wetness = wet
                .zip(gpu.wet_offset(id))
                .map(|(wetness, first)| wetness.with_first_vertex(first));

            for (submesh, described) in gpu.model().mesh(mesh).submeshes.iter().enumerate() {
                let outline = gpu.outline(described.material);
                let material = restyle
                    .and_then(|restyle| restyle.material(described.material))
                    .unwrap_or_else(|| gpu.material(described.material));
                let order = described
                    .material
                    .map_or(0, |id| gpu.model().material(id).render_order());
                let layers = layers
                    | described.material.map_or(0, |id| {
                        materials::character_layers(gpu.model().material(id))
                    });
                for (pass_material, pickable) in [(material, true)]
                    .into_iter()
                    .chain(outline.map(|outline| (outline, false)))
                {
                    let chosen = pickable
                        && described
                            .material
                            .is_some_and(|material| selected.contains(&material));
                    let mut draw = Draw::new(uploaded, pass_material)
                        .selected(chosen)
                        .submesh(submesh as u32)
                        .transform(transform)
                        .object_id(index as u32)
                        .pickable(pickable)
                        .order(order)
                        .layers(layers)
                        .light_channels(light_channels);
                    if let Some(skin) = skin {
                        draw = draw.skin(skin);
                    }
                    if let Some(figure) = figure {
                        draw = draw.figure(figure);
                    }
                    if let Some((first, count)) = weights {
                        draw = draw.morph_weights(first, count);
                    }
                    if let Some(drape) = drape {
                        draw = draw.with_drape(drape);
                    }
                    if let Some(wetness) = wetness {
                        draw = draw.wet(wetness);
                    }
                    frame.draw(draw);
                }
            }
        }
    }
}

impl GpuModel {
    pub fn drawing<'a>(&'a self, instance: &'a Instance) -> ModelDrawing<'a> {
        ModelDrawing {
            gpu: self,
            instance,
            figure: None,
            layers: DEFAULT_LAYERS,
            wet: None,
            drapes: Vec::new(),
            restyle: None,
            light_channels: DEFAULT_LIGHT_CHANNELS,
            selected: &[],
        }
    }

    pub fn draw(&self, frame: &mut Frame, instance: &Instance) {
        self.drawing(instance).draw_into(frame);
    }

    #[must_use]
    pub fn lights(&self, instance: &Instance) -> Vec<GpuLight> {
        self.model()
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| Some((NodeId(index as u32), node.light?)))
            .map(|(node, light)| {
                let placement = instance.transform() * instance.world(node);
                let position = placement.w_axis.truncate();
                let direction = -placement.z_axis.truncate().normalize_or(Vec3::NEG_Y);
                let light = self.model().light(light);
                let placed = match light.kind {
                    LightKind::Directional => GpuLight::directional(direction),
                    LightKind::Point { range } => GpuLight::point(position, range),
                    LightKind::Spot {
                        range,
                        inner_angle,
                        outer_angle,
                    } => GpuLight::spot(position, direction, range, inner_angle, outer_angle),
                };
                placed.color(light.color).intensity(light.intensity)
            })
            .collect()
    }

    #[must_use]
    pub fn camera(&self, instance: &Instance, node: NodeId, aspect: f32) -> Option<Camera> {
        let camera = self.model().node(node).camera?;
        let projection = match self.model().camera(camera).projection {
            Projection::Perspective {
                y_fov,
                aspect: camera_aspect,
                near,
                ..
            } => perspective_reversed_infinite(y_fov, camera_aspect.unwrap_or(aspect), near),
            Projection::Orthographic {
                width,
                height,
                near,
                far,
            } => orthographic_reversed(
                -width * 0.5,
                width * 0.5,
                -height * 0.5,
                height * 0.5,
                near,
                far,
            ),
        };
        let view = (instance.transform() * instance.world(node)).inverse();
        Some(Camera { view, projection })
    }

    #[must_use]
    pub fn node_at(&self, object_id: u32) -> Option<NodeId> {
        let id = NodeId(object_id);
        (id.index() < self.model().nodes.len()).then_some(id)
    }
}
