use rikka_physics_collision::{Filter, Material};
use rikka_physics_geometry::Shape;

#[must_use]
#[derive(Clone, Debug)]
pub struct ShapeDef {
    pub(crate) shape: Shape,
    pub(crate) materials: Vec<Material>,
    pub(crate) filter: Filter,
    pub(crate) sensor: bool,
    pub(crate) density: f32,
    pub(crate) contact_events: bool,
    pub(crate) hit_events: bool,
    pub(crate) sensor_events: bool,
    pub(crate) contact_response: bool,
}

impl<S: Into<Shape>> From<S> for ShapeDef {
    fn from(shape: S) -> Self {
        Self::new(shape)
    }
}

impl ShapeDef {
    pub fn new(shape: impl Into<Shape>) -> Self {
        Self {
            shape: shape.into(),
            materials: vec![Material::default()],
            filter: Filter::default(),
            sensor: false,
            density: 1000.0,
            contact_events: true,
            hit_events: false,
            sensor_events: true,
            contact_response: true,
        }
    }

    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    pub fn material(mut self, material: Material) -> Self {
        self.materials[0] = material;
        self
    }

    pub fn materials(mut self, materials: impl IntoIterator<Item = Material>) -> Self {
        let materials: Vec<Material> = materials.into_iter().collect();
        if !materials.is_empty() {
            self.materials = materials;
        }
        self
    }

    pub fn filter(mut self, filter: Filter) -> Self {
        self.filter = filter;
        self
    }

    pub fn sensor(mut self, sensor: bool) -> Self {
        self.sensor = sensor;
        self
    }

    pub fn density(mut self, density: f32) -> Self {
        self.density = density;
        self
    }

    pub fn contact_events(mut self, enabled: bool) -> Self {
        self.contact_events = enabled;
        self
    }

    pub fn hit_events(mut self, enabled: bool) -> Self {
        self.hit_events = enabled;
        self
    }

    pub fn sensor_events(mut self, enabled: bool) -> Self {
        self.sensor_events = enabled;
        self
    }

    pub fn contact_response(mut self, enabled: bool) -> Self {
        self.contact_response = enabled;
        self
    }
}
