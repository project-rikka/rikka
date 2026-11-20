use rikka_model_core::Instance;
use rikka_model_core::glam::Vec3;

use super::avatar::Vrm;
use super::expression::ExpressionName;
use super::runtime::Avatar;

pub trait Expressive {
    fn expression(&self, name: &ExpressionName) -> f32;

    fn set_expression(&mut self, name: impl Into<ExpressionName>, weight: f32);

    fn clear_expressions(&mut self);

    fn look_at_target(&self) -> Option<Vec3>;

    fn set_look_at_target(&mut self, target: Option<Vec3>);
}

impl Expressive for Instance {
    fn expression(&self, name: &ExpressionName) -> f32 {
        let Some(index) = expression_index(self, name) else {
            return 0.0;
        };
        self.rig_as::<Avatar>()
            .map_or(0.0, |avatar| avatar.weight(index))
    }

    fn set_expression(&mut self, name: impl Into<ExpressionName>, weight: f32) {
        let Some(index) = expression_index(self, &name.into()) else {
            return;
        };
        if let Some(avatar) = self.rig_as_mut::<Avatar>() {
            avatar.set_weight(index, weight);
        }
    }

    fn clear_expressions(&mut self) {
        if let Some(avatar) = self.rig_as_mut::<Avatar>() {
            avatar.clear_weights();
        }
    }

    fn look_at_target(&self) -> Option<Vec3> {
        self.rig_as::<Avatar>().and_then(Avatar::look_at)
    }

    fn set_look_at_target(&mut self, target: Option<Vec3>) {
        if let Some(avatar) = self.rig_as_mut::<Avatar>() {
            avatar.set_look_at(target);
        }
    }
}

fn expression_index(instance: &Instance, name: &ExpressionName) -> Option<usize> {
    instance
        .model()
        .extensions
        .get::<Vrm>()?
        .expression_index(name)
}
