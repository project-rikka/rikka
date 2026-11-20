use glam::{Mat4, Vec3};

use crate::camera::Camera;
use crate::gpu::{GpuInstance, NONE_INDEX, figure_flags, shadow_flags};
use crate::light::{Light, LightKind, Placed};
use crate::look::Figure;
use crate::look::gpu::TRACED_LIGHTS;
use crate::shadow::cache::{self, Reuse};
use crate::shadow::frusta::{self, Projected};
use crate::shadow::{
    Atlas, AtlasAllocator, Bounds, CASCADES, LARGEST_ATLAS, LARGEST_FIGURE_ATLAS, ShadowCache,
    ShadowView,
};

const SPLIT_NEAR: f32 = 0.5;

const SPLIT_LAMBDA: f32 = 0.85;

const SMALLEST_TILE: u32 = 128;

const HEAD_REACH: f32 = 1.4;

const CASCADES_GROUP: u8 = 0;

const LOCAL: u8 = 1;

pub(crate) struct Key {
    pub figure: u32,
    pub light: Light,
    pub turned: bool,
}

pub(crate) struct PlanInput<'a> {
    pub camera: &'a Camera,
    pub distance: f32,
    pub lights: &'a [Light],
    pub leading: Option<usize>,
    pub keys: &'a [Key],
    pub figures: &'a [Figure],
    pub bounds: &'a [Bounds],
    pub instances: &'a [GpuInstance],
    pub cacheable: bool,
    pub tinted: bool,
    pub traced: Traced,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Traced {
    pub figures: bool,
    keys: [u32; TRACED_LIGHTS],
}

impl Traced {
    pub(crate) const NONE: Self = Self {
        figures: false,
        keys: [NONE_INDEX; TRACED_LIGHTS],
    };

    pub(crate) fn of(lights: &[Light], leading: Option<usize>, keys: &[Key]) -> Self {
        let mut traced = Self::NONE;
        let mut count = 0;
        for directional in [true, false] {
            for (index, light) in lights.iter().enumerate() {
                if light.kind.is_directional() != directional || !light.shadow.casts() {
                    continue;
                }
                if count == TRACED_LIGHTS {
                    return traced;
                }
                count += 1;
                traced.figures |= leading == Some(index);
            }
        }
        let mut traced_keys = 0;
        for (index, key) in keys.iter().enumerate() {
            if !key.turned || !key.light.shadow.casts() {
                continue;
            }
            if count == TRACED_LIGHTS {
                break;
            }
            count += 1;
            traced.keys[traced_keys] = index as u32;
            traced_keys += 1;
        }
        traced
    }

    fn key(&self, index: usize) -> bool {
        self.keys.contains(&(index as u32))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Light(u32),
    Figure(u32),
    Key(u32),
}

impl Source {
    fn atlas(self) -> Atlas {
        match self {
            Self::Light(_) => Atlas::Main,
            Self::Figure(_) | Self::Key(_) => Atlas::Figures,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Request {
    source: Source,
    views: u32,
    tile: u32,
    group: u8,
    priority: f32,
    key: u64,
    casters: Option<u64>,
    cached: bool,
    first: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FigurePlan {
    pub shadow: u32,
    pub flags: u32,
}

impl Default for FigurePlan {
    fn default() -> Self {
        Self {
            shadow: NONE_INDEX,
            flags: 0,
        }
    }
}

#[derive(Default)]
pub(crate) struct Plan {
    pub views: Vec<ShadowView>,
    pub lights: Vec<Placed>,
    pub keys: Vec<Placed>,
    pub figures: Vec<FigurePlan>,
    requests: Vec<Request>,
    order: Vec<u32>,
    origins: Vec<(u32, u32)>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Planned {
    pub side: u32,
    pub figures: u32,
}

pub(crate) struct Allocators<'a> {
    pub main: &'a mut AtlasAllocator,
    pub figures: Option<&'a mut AtlasAllocator>,
}

fn is_local(light: &Light) -> bool {
    !light.kind.is_directional()
}

fn importance(camera: &Camera, light: &Light) -> f32 {
    let reach = light.kind.range() + light.kind.extent();
    reach / camera.position().distance(light.kind.position()).max(1e-3)
}

fn turned(keys: &[Key], figure: u32) -> bool {
    keys.iter().any(|key| key.figure == figure && key.turned)
}

fn side_for(requests: &[Request], atlas: Atlas, largest_side: u32) -> u32 {
    let (area, largest) = requests
        .iter()
        .filter(|request| request.views > 0 && request.source.atlas() == atlas)
        .fold((0u64, 0u64), |(area, largest), request| {
            let tile = u64::from(request.tile);
            (
                area + u64::from(request.views) * tile * tile,
                largest.max(tile),
            )
        });
    match area {
        0 => 0,
        _ => ((area as f64).sqrt().ceil() as u64)
            .max(largest)
            .next_power_of_two()
            .min(u64::from(largest_side)) as u32,
    }
}

fn figure_box(instances: &[GpuInstance], figure: u32, turn: Mat4) -> Bounds {
    let mut fitted = Bounds::EMPTY;
    for instance in instances {
        if instance.figure != figure || instance.index_count == 0 || !instance.radius.is_finite() {
            continue;
        }
        let center = turn.transform_point3(Vec3::from_array(instance.center));
        fitted.enclose(center, instance.radius);
    }
    fitted
}

impl Plan {
    pub(crate) fn clear(&mut self, lights: usize, keys: usize, figures: usize) {
        self.views.clear();
        self.requests.clear();
        self.lights.clear();
        self.lights.resize(lights, Placed::default());
        self.keys.clear();
        self.keys.resize(keys, Placed::default());
        self.figures.clear();
        self.figures.resize(figures + 1, FigurePlan::default());
    }

    pub(crate) fn size(&mut self, input: &PlanInput<'_>) -> Planned {
        self.clear(input.lights.len(), input.keys.len(), input.figures.len());
        for (index, light) in input.lights.iter().enumerate() {
            if !light.shadow.casts() {
                continue;
            }
            let local = is_local(light);
            self.push(Request {
                source: Source::Light(index as u32),
                views: light.shadow_views(CASCADES),
                tile: light.shadow.resolution(),
                group: if local { LOCAL } else { CASCADES_GROUP },
                priority: if local {
                    importance(input.camera, light)
                } else {
                    f32::INFINITY
                },
                key: 0,
                casters: None,
                cached: false,
                first: 0,
            });
        }
        let leading = input
            .leading
            .filter(|&leading| input.lights[leading].shadow.casts());
        for (index, figure) in input.figures.iter().enumerate() {
            let id = index as u32 + 1;
            let bounded = input
                .bounds
                .get(id as usize)
                .is_some_and(|bounds| !bounds.is_empty());
            if !figure.shadow.casts() || !bounded {
                continue;
            }
            let source = match turned(input.keys, id) {
                true => match input
                    .keys
                    .iter()
                    .position(|key| key.figure == id && key.light.shadow.casts())
                {
                    Some(key) if input.traced.key(key) => continue,
                    Some(key) => Source::Key(key as u32),
                    None => continue,
                },
                false if leading.is_some() && !input.traced.figures => Source::Figure(id),
                false => continue,
            };
            let distance = figure.head.map_or(f32::MAX, |head| {
                input.camera.position().distance(head.center)
            });
            self.push(Request {
                source,
                views: 1 + u32::from(figure.head.is_some()),
                tile: figure.shadow.resolution(),
                group: 0,
                priority: 1.0 / distance.max(1e-3),
                key: 0,
                casters: None,
                cached: false,
                first: 0,
            });
        }
        self.fit_budget(Atlas::Main, LARGEST_ATLAS);
        self.fit_budget(Atlas::Figures, LARGEST_FIGURE_ATLAS);
        Planned {
            side: side_for(&self.requests, Atlas::Main, LARGEST_ATLAS),
            figures: side_for(&self.requests, Atlas::Figures, LARGEST_FIGURE_ATLAS),
        }
    }

    fn push(&mut self, request: Request) {
        if request.views > 0 && request.tile > 0 {
            self.requests.push(request);
        }
    }

    fn area(&self, atlas: Atlas) -> u64 {
        self.requests
            .iter()
            .filter(|request| request.source.atlas() == atlas)
            .map(|request| u64::from(request.views) * u64::from(request.tile).pow(2))
            .sum()
    }

    fn least(&self, atlas: Atlas, shrinkable: bool) -> Option<usize> {
        self.requests
            .iter()
            .enumerate()
            .filter(|(_, request)| {
                request.source.atlas() == atlas
                    && request.views > 0
                    && (!shrinkable || request.tile > SMALLEST_TILE)
            })
            .max_by(|(_, a), (_, b)| {
                a.group
                    .cmp(&b.group)
                    .then_with(|| b.priority.total_cmp(&a.priority))
            })
            .map(|(index, _)| index)
    }

    fn fit_budget(&mut self, atlas: Atlas, largest: u32) {
        let budget = u64::from(largest).pow(2);
        while self.area(atlas) > budget {
            if let Some(index) = self.least(atlas, true) {
                self.requests[index].tile /= 2;
            } else if let Some(index) = self.least(atlas, false) {
                self.requests[index].views = 0;
            } else {
                break;
            }
        }
    }

    pub(crate) fn place(
        &mut self,
        input: &PlanInput<'_>,
        allocators: Allocators<'_>,
        cache: &mut ShadowCache,
        generation: u64,
    ) {
        let Allocators { main, mut figures } = allocators;
        main.reset();
        if let Some(figures) = figures.as_deref_mut() {
            figures.reset();
        }
        cache.begin(generation);
        let mut first = 0;
        for request in &mut self.requests {
            request.first = first;
            first += request.views;
        }
        self.origins.clear();
        self.origins.resize(first as usize, (0, 0));

        for request in &mut self.requests {
            let Source::Light(index) = request.source else {
                continue;
            };
            let light = &input.lights[index as usize];
            if !is_local(light) || !input.cacheable || request.views == 0 {
                continue;
            }
            let reuse = Reuse {
                key: cache::light_key(light, request.tile),
                casters: cache::casters(light, input.instances),
            };
            (request.key, request.casters) = (reuse.key, reuse.casters);
            let Some(origins) = cache.cached(&reuse, request.tile) else {
                continue;
            };
            if origins.len() != request.views as usize
                || !origins
                    .iter()
                    .all(|&origin| main.reserve(origin, request.tile))
            {
                continue;
            }
            request.cached = true;
            let at = request.first as usize;
            self.origins[at..at + origins.len()].copy_from_slice(origins);
        }

        self.order.clear();
        self.order.extend(
            (0..self.requests.len() as u32).filter(|&index| !self.requests[index as usize].cached),
        );
        let requests = &self.requests;
        self.order.sort_unstable_by_key(|&index| {
            (std::cmp::Reverse(requests[index as usize].tile), index)
        });
        for &index in &self.order {
            let request = &mut self.requests[index as usize];
            let allocator = match (request.source.atlas(), figures.as_deref_mut()) {
                (Atlas::Main, _) => &mut *main,
                (Atlas::Figures, Some(figures)) => figures,
                (Atlas::Figures, None) => {
                    request.views = 0;
                    continue;
                }
            };
            let wanted = request.views;
            for view in 0..wanted {
                match allocator.allocate(request.tile) {
                    Some(origin) => self.origins[(request.first + view) as usize] = origin,
                    None => {
                        request.views = view;
                        break;
                    }
                }
            }
            let whole = match request.source {
                Source::Light(_) => wanted == 6,
                Source::Figure(_) | Source::Key(_) => false,
            };
            if whole && request.views < wanted {
                request.views = 0;
            }
        }

        for index in 0..self.requests.len() {
            self.lay(input, index);
        }
        let leading = input.leading.map(|leading| self.lights[leading]);
        for (index, key) in input.keys.iter().enumerate() {
            if key.turned && input.traced.key(index) {
                self.keys[index] = Placed::unmapped(index as u32);
            } else if !key.turned
                && let Some(leading) = leading
            {
                self.keys[index] = leading;
            }
        }
        for request in &self.requests {
            if request.casters.is_some() && request.views > 0 {
                let views = &self.views[request.first as usize..][..request.views as usize];
                cache.keep(
                    &Reuse {
                        key: request.key,
                        casters: request.casters,
                    },
                    request.tile,
                    views,
                );
            }
        }
        cache.finish();
    }

    fn lay(&mut self, input: &PlanInput<'_>, index: usize) {
        let request = self.requests[index];
        let start = self.views.len() as u32;
        self.requests[index].first = start;
        if request.views == 0 {
            return;
        }
        let origins = request.first as usize;
        let mut flags = 0;
        if request.cached {
            flags |= shadow_flags::CACHED;
        }
        let mapped = self
            .requests
            .iter()
            .any(|request| matches!(request.source, Source::Figure(_)) && request.views > 0);
        match request.source {
            Source::Light(light_index) => {
                if input.tinted {
                    flags |= shadow_flags::TINTED;
                }
                let light = &input.lights[light_index as usize];
                if input.leading == Some(light_index as usize) && mapped {
                    flags |= shadow_flags::WITHOUT_MAPPED_FIGURES;
                }
                let near = input.camera.near();
                let far = input.distance.max(near * 2.0);
                let splits =
                    frusta::cascade_splits(near.max(SPLIT_NEAR).min(far * 0.5), far, SPLIT_LAMBDA);
                for view in 0..request.views {
                    let (projected, split_depth, perspective) = match light.kind {
                        LightKind::Directional { direction } => {
                            let start = match view {
                                0 => near,
                                i => splits[i as usize - 1],
                            };
                            let end = splits[view as usize];
                            (
                                frusta::cascade_view(
                                    input.camera,
                                    direction,
                                    start,
                                    end,
                                    request.tile,
                                ),
                                end,
                                false,
                            )
                        }
                        LightKind::Spot {
                            position,
                            direction,
                            range,
                            outer_angle,
                            ..
                        } => (
                            frusta::spot_view(
                                position,
                                direction,
                                range,
                                outer_angle,
                                request.tile,
                            ),
                            0.0,
                            true,
                        ),
                        _ => {
                            let range = light.kind.range() + light.kind.extent();
                            let faces =
                                frusta::cube_views(light.kind.position(), range, request.tile);
                            (faces[view as usize], 0.0, true)
                        }
                    };
                    let flags = flags
                        | match perspective {
                            true => shadow_flags::PERSPECTIVE,
                            false => 0,
                        };
                    self.views.push(view_of(
                        projected,
                        Placement {
                            atlas: Atlas::Main,
                            origin: self.origins[origins + view as usize],
                            tile: request.tile,
                        },
                        Role {
                            split_depth,
                            figure: 0,
                            channels: light.channels,
                            flags,
                        },
                    ));
                }
                self.lights[light_index as usize] = Placed {
                    shadow: start,
                    shadow_count: request.views,
                    ..self.lights[light_index as usize]
                };
            }
            Source::Figure(figure) => {
                let Some(leading) = input.leading else {
                    return;
                };
                let light = input.lights[leading];
                self.lay_figure(input, &request, figure, &light, flags);
                let placed = &mut self.lights[leading];
                if placed.character_count == 0 {
                    placed.characters = start;
                }
                placed.character_count += request.views;
                self.figures[figure as usize] = FigurePlan {
                    shadow: start,
                    flags: figure_flags::MAPPED,
                };
            }
            Source::Key(key_index) => {
                let key = &input.keys[key_index as usize];
                let (figure, light) = (key.figure, key.light);
                self.lay_figure(input, &request, figure, &light, flags);
                self.keys[key_index as usize] = Placed {
                    shadow: start,
                    shadow_count: request.views,
                    ..Placed::default()
                };
                self.figures[figure as usize] = FigurePlan {
                    shadow: start,
                    flags: figure_flags::TURNED_KEY,
                };
            }
        }
    }

    fn lay_figure(
        &mut self,
        input: &PlanInput<'_>,
        request: &Request,
        figure: u32,
        light: &Light,
        flags: u32,
    ) {
        let direction = light.kind.direction();
        let turn = frusta::light_turn(direction);
        let body = figure_box(input.instances, figure, turn);
        let flags = flags | shadow_flags::FIGURE;
        let detailed = request.views > 1;
        let origin = request.first as usize;
        let role = |flags| Role {
            split_depth: 0.0,
            figure,
            channels: light.channels,
            flags,
        };
        let placement = |view: usize| Placement {
            atlas: Atlas::Figures,
            origin: self.origins[origin + view],
            tile: request.tile,
        };
        let body_view = view_of(
            frusta::figure_view(body, direction, request.tile),
            placement(0),
            role(match detailed {
                true => flags | shadow_flags::DETAILED,
                false => flags,
            }),
        );
        let head = input.figures[figure as usize - 1].head;
        let head_view = match (detailed, head) {
            (true, Some(head)) => {
                let center = turn.transform_point3(head.center);
                let reach = head.radius * HEAD_REACH;
                let detail = Bounds {
                    low: Vec3::new(center.x - reach, center.y - reach, body.low.z),
                    high: Vec3::new(center.x + reach, center.y + reach, body.high.z),
                };
                Some(view_of(
                    frusta::figure_view(detail, direction, request.tile),
                    placement(1),
                    role(flags),
                ))
            }
            _ => None,
        };
        self.views.push(body_view);
        self.views.extend(head_view);
    }
}

struct Placement {
    atlas: Atlas,
    origin: (u32, u32),
    tile: u32,
}

struct Role {
    split_depth: f32,
    figure: u32,
    channels: u32,
    flags: u32,
}

fn view_of(projected: Projected, placement: Placement, role: Role) -> ShadowView {
    let (depth_bias, normal_bias) = frusta::biases(projected.world_texel);
    ShadowView {
        atlas: placement.atlas,
        view_projection: projected.view_projection,
        origin: placement.origin,
        tile: placement.tile,
        split_depth: role.split_depth,
        depth_bias,
        normal_bias,
        figure: role.figure,
        channels: role.channels,
        flags: role.flags,
        near: projected.near,
        projection_scale: projected.projection_scale,
        depth_range: projected.depth_range,
        world_texel: projected.world_texel,
    }
}
