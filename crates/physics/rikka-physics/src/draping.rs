use rikka_model_core::NodeId;
use rikka_physics_cloth::Drape;
use rikka_renderer_3d::Frame;

pub(crate) fn to_frame(
    frame: &mut Frame,
    drapes: &[Drape],
) -> Vec<(NodeId, rikka_renderer_3d::Drape)> {
    drapes
        .iter()
        .map(|drape| (drape.node, frame.drape(&drape.buffer, &drape.ready)))
        .collect()
}
