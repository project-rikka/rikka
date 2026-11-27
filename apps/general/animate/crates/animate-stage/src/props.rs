use std::sync::Arc;

use animate_body::MorphWeights;
use animate_cast::{Prop, PropId, PropRole, PropTrack, TakePlayback, frame_of};
use animate_look::Name;
use animate_prop::PropLevels;
use rikka_model::glam::Mat4;
use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_model::{Bounds, Instance, MaterialId};
use rikka_model_gpu::{GpuModel, Prepared, UploadOptions};
use rikka_renderer::Renderer;
use rikka_renderer_3d::{DEFAULT_LIGHT_CHANNELS, Frame};

use crate::actor::{Actor, perform};
use crate::render::Render;
use crate::scene::Scene;
use crate::unkeyed::Drafts;
use crate::{Stage, StageError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropFrame {
    pub role: PropRole,
    pub view_projection: Mat4,
    pub parent: Mat4,
    pub levels: PropLevels,
}

impl Stage {
    pub fn set_props<'a>(&mut self, tracks: impl IntoIterator<Item = &'a PropTrack>) {
        let members: Vec<PropMember> = tracks.into_iter().map(PropMember::of).collect();
        let unchanged = members.len() == self.scene.props.len()
            && members
                .iter()
                .zip(&self.scene.props)
                .all(|(one, two)| one.same_as(two));
        if unchanged {
            return;
        }
        let alive = |id: &PropId| members.iter().any(|member| member.id() == *id);
        self.supplied.retain(alive);
        self.prop_prepared.retain(|id, _| alive(id));
        self.change().props = members;
    }

    pub fn supply_prop(&mut self, id: PropId, prepared: Prepared) {
        self.supplied.insert(id);
        self.prop_prepared.insert(id, prepared);
        self.change();
    }

    pub fn lacks_prop(&self, id: PropId) -> bool {
        !self.supplied.contains(&id)
    }

    pub fn highlight_prop(&mut self, prop: PropId, materials: &[u32]) {
        let wanted = (!materials.is_empty()).then(|| {
            let ids: Arc<[MaterialId]> = materials.iter().copied().map(MaterialId).collect();
            (prop, ids)
        });
        if self.scene.prop_highlight != wanted {
            self.change().prop_highlight = wanted;
        }
    }

    pub fn show_prop(&mut self, role: Option<PropRole>) {
        if self.scene.chosen_prop != role {
            self.change().chosen_prop = role;
            self.outputs.prop_frame = None;
        }
    }

    pub fn prop_frame(&self) -> Option<&PropFrame> {
        self.outputs.prop_frame.as_ref()
    }
}

impl Render {
    pub(crate) fn sync_props(&mut self, scene: &Scene) {
        let alive = |id: PropId| scene.props.iter().any(|member| member.id() == id);
        self.props.retain(|actor| alive(actor.id()));
        let gone: Vec<PropId> = self
            .prop_gpus
            .keys()
            .copied()
            .filter(|id| !alive(*id))
            .collect();
        for id in gone {
            self.prop_gpus.remove(&id);
            self.dressing.forget_prop(id);
        }
        self.prop_prepared.retain(|id, _| alive(*id));
        self.unloadable_props.retain(|id| alive(*id));
        for actor in &mut self.props {
            if let Some(member) = scene.props.iter().find(|member| member.id() == actor.id()) {
                actor.recast(member);
            }
        }
    }

    pub(crate) fn prepare_props(&mut self, scene: &Scene) {
        for member in &scene.props {
            let id = member.id();
            if !self.prop_gpus.contains_key(&id) && !self.prop_prepared.contains_key(&id) {
                let model = Arc::clone(&member.prop.model);
                self.prop_prepared
                    .insert(id, Prepared::new(model, &UploadOptions::default()));
            }
        }
    }

    pub(crate) fn upload_props(&mut self, renderer: &mut Renderer, scene: &Scene) {
        let gpus = &self.prop_gpus;
        self.prop_prepared.retain(|id, _| !gpus.contains_key(id));
        for member in &scene.props {
            let id = member.id();
            if self.prop_gpus.contains_key(&id) || self.unloadable_props.contains(&id) {
                continue;
            }
            let Some(prepared) = self.prop_prepared.remove(&id) else {
                continue;
            };
            match GpuModel::upload_prepared(renderer, prepared) {
                Ok(gpu) => {
                    self.props
                        .push(PropActor::cast(member.clone(), gpu.instance()));
                    self.prop_gpus.insert(id, gpu);
                    self.dirty = true;
                }
                Err(error) => {
                    self.unloadable_props.insert(id);
                    self.fail(StageError::Upload(error));
                }
            }
        }
    }

    pub(crate) fn place_props(&mut self, scene: &Scene, time: f32) {
        for actor in &mut self.props {
            actor.place(&scene.drafts, &self.actors, time);
        }
    }

    pub(crate) fn dressed_props(&self) -> Vec<(PropId, Name)> {
        self.props
            .iter()
            .filter(|actor| actor.visible)
            .map(|actor| (actor.id(), actor.look_name))
            .collect()
    }

    pub(crate) fn project_prop(&self, scene: &Scene, view_projection: Mat4) -> Option<PropFrame> {
        let role = scene.chosen_prop?;
        let actor = self
            .props
            .iter()
            .find(|actor| actor.member.prop.role() == role)?;
        Some(PropFrame {
            role,
            view_projection,
            parent: actor.parent,
            levels: actor.levels,
        })
    }

    pub(crate) fn prop_bounds(&self) -> Vec<Bounds> {
        self.props
            .iter()
            .filter(|actor| actor.visible)
            .map(|actor| actor.instance.bounds())
            .collect()
    }

    pub(crate) fn draw_props(
        &self,
        renderer: &mut Renderer,
        frame: &mut Frame,
        scene: &Scene,
    ) -> Option<StageError> {
        let mut failure = None;
        for actor in self.props.iter().filter(|actor| actor.visible) {
            let Some(gpu) = self.prop_gpus.get(&actor.id()) else {
                continue;
            };
            if let Err(error) = gpu.refresh_materials(renderer, &actor.instance) {
                failure = Some(StageError::Upload(error));
            }
            let drawing = gpu
                .drawing(&actor.instance)
                .light_channels(DEFAULT_LIGHT_CHANNELS);
            match &scene.prop_highlight {
                Some((prop, materials)) if *prop == actor.id() => {
                    drawing.selected(materials).draw_into(frame);
                }
                _ => drawing.draw_into(frame),
            }
        }
        failure
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PropMember {
    pub(crate) prop: Arc<Prop>,
    pub(crate) playback: Option<Arc<TakePlayback>>,
}

impl PropMember {
    pub(crate) fn of(track: &PropTrack) -> Self {
        Self {
            prop: Arc::clone(&track.prop),
            playback: track.playback.clone(),
        }
    }

    pub(crate) fn id(&self) -> PropId {
        self.prop.id
    }

    pub(crate) fn same_as(&self, other: &PropMember) -> bool {
        Arc::ptr_eq(&self.prop, &other.prop)
            && match (&self.playback, &other.playback) {
                (Some(one), Some(two)) => Arc::ptr_eq(one, two),
                (one, two) => one.is_none() && two.is_none(),
            }
    }

    pub(crate) fn levels_at(&self, drafts: &Drafts, time: f32) -> PropLevels {
        let mut levels = self.prop.motion.levels_at(time * FRAMES_PER_SECOND);
        if let Some(held) = drafts.props.at(self.prop.role(), frame_of(time)) {
            held.apply(|setting, value| levels.set(setting, value));
        }
        levels
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Placed {
    world: Mat4,
    time: Option<f32>,
}

pub(crate) struct PropActor {
    pub(crate) member: PropMember,
    pub(crate) look_name: Name,
    pub(crate) instance: Instance,
    pub(crate) visible: bool,
    parent: Mat4,
    levels: PropLevels,
    placed: Option<Placed>,
}

impl PropActor {
    pub(crate) fn cast(member: PropMember, instance: Instance) -> Self {
        Self {
            look_name: Name::new(&member.prop.name),
            member,
            instance,
            visible: true,
            parent: Mat4::IDENTITY,
            levels: PropLevels::default(),
            placed: None,
        }
    }

    pub(crate) fn id(&self) -> PropId {
        self.member.id()
    }

    pub(crate) fn recast(&mut self, member: &PropMember) {
        if !self.member.same_as(member) {
            self.look_name = Name::new(&member.prop.name);
            self.member = member.clone();
            self.placed = None;
        }
    }

    pub(crate) fn place(&mut self, drafts: &Drafts, actors: &[Actor], time: f32) {
        let levels = self.member.levels_at(drafts, time);
        let anchor = self
            .member
            .prop
            .kind
            .attaches()
            .then(|| self.member.prop.motion.anchor_at(time * FRAMES_PER_SECOND))
            .flatten()
            .and_then(|anchor| {
                actors
                    .iter()
                    .find(|actor| actor.name == anchor.figure)?
                    .bone_world(&anchor.bone)
            });
        self.parent = anchor.unwrap_or(Mat4::IDENTITY);
        self.levels = levels;
        let world = self.parent * levels.transform();
        self.visible = levels.visible;
        let placed = Placed {
            world,
            time: self.member.playback.as_ref().map(|_| time),
        };
        if self.placed == Some(placed) {
            return;
        }
        if self.placed.is_none() || placed.time.is_some() {
            perform(
                self.member.playback.as_deref(),
                None,
                &MorphWeights::default(),
                &mut self.instance,
                time,
            );
        }
        self.instance.set_transform(world);
        self.instance.refresh();
        self.placed = Some(placed);
    }
}
