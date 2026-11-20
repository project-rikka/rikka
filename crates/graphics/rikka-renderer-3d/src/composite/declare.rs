use crate::composite::layers::{LayerImages, Reading};
use crate::composite::{Composite, CompositeInput, CompositeNode, Glow, Operation};
use crate::effect::{EffectNode, Sockets, Source, declare_effect};
use crate::graph::{FrameGraph, ImageId};
use crate::post::{Exposed, GlowInputs, exposure, finish, focus, glow, motion};
use crate::record::{SceneImages, Stage};

#[derive(Clone, Copy, Debug)]
pub(crate) struct NodeOutput {
    image: ImageId,
    exposed: Option<Exposed>,
}

impl NodeOutput {
    fn plain(image: ImageId) -> Self {
        Self {
            image,
            exposed: None,
        }
    }
}

struct Links<'s> {
    scene: &'s SceneImages,
    resolved: ImageId,
    layers: LayerImages,
}

impl Links<'_> {
    fn image(
        &mut self,
        graph: &mut FrameGraph<'_>,
        outputs: &[Option<NodeOutput>],
        input: CompositeInput,
        reading: Option<Reading>,
    ) -> ImageId {
        match input {
            CompositeInput::Node(index) => {
                outputs[index]
                    .expect("composite nodes read declared nodes")
                    .image
            }
            CompositeInput::Layer(layer) => {
                self.layers
                    .image(graph, self.scene, self.resolved, layer, reading)
            }
        }
    }

    fn socket(
        &mut self,
        graph: &mut FrameGraph<'_>,
        outputs: &[Option<NodeOutput>],
        node: &CompositeNode,
        name: &str,
        reading: Option<Reading>,
    ) -> Option<ImageId> {
        let input = node.input(name)?;
        Some(self.image(graph, outputs, input, reading))
    }
}

fn exposed_by(outputs: &[Option<NodeOutput>], node: Option<usize>) -> Option<Exposed> {
    outputs[node?]?.exposed
}

pub(crate) fn declare<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    scene: &SceneImages,
    resolved: ImageId,
    first_state: Option<ImageId>,
    outputs: &mut Vec<Option<NodeOutput>>,
) -> ImageId {
    let composite = &stage
        .look
        .expect("composites belong to looked frames")
        .composite;
    let plan = stage.composite;
    outputs.clear();
    outputs.resize(composite.nodes.len(), None);
    let mut links = Links {
        scene,
        resolved,
        layers: LayerImages::new(stage.frame.cut),
    };
    for (index, (node, planned)) in composite.nodes.iter().zip(plan.nodes()).enumerate() {
        if !planned.reachable {
            continue;
        }
        let reading = planned.reading.and_then(|at| {
            exposed_by(outputs, Some(at)).map(|exposed| Reading { node: at, exposed })
        });
        let context = exposed_by(outputs, planned.context.filter(|&at| at != index));
        let image = match node.operation {
            Operation::Effect(_) => resolved,
            _ => links
                .socket(graph, outputs, node, Composite::IMAGE, reading)
                .expect("validated nodes wire their image"),
        };
        let output = match &node.operation {
            Operation::Exposure(settings) => {
                let exposed = match exposure::fixed(settings, stage.frame.exposure) {
                    Some(value) => Exposed::Fixed(value),
                    None => Exposed::Metered(
                        first_state
                            .filter(|_| plan.first_exposure() == Some(index))
                            .unwrap_or_else(|| exposure::state(graph, node.key)),
                    ),
                };
                NodeOutput {
                    image: exposure::declare(graph, stage, scene, image, settings, exposed),
                    exposed: Some(exposed),
                }
            }
            Operation::MotionBlur(blur) => {
                NodeOutput::plain(motion::declare(graph, stage, scene, image, blur))
            }
            Operation::DepthOfField(lens) => {
                NodeOutput::plain(focus::declare(graph, stage, scene, image, lens))
            }
            Operation::Glow(settings) => {
                let source = match settings.glows() {
                    true => links.socket(graph, outputs, node, Glow::SOURCE, reading),
                    false => None,
                };
                let inputs = GlowInputs {
                    image,
                    source,
                    block: planned.block,
                    exposed: context,
                };
                NodeOutput::plain(glow::declare(graph, stage, scene, settings, inputs))
            }
            Operation::Grade(_) => {
                NodeOutput::plain(finish::grade(graph, image, planned.block, context))
            }
            Operation::Tonemap(tonemap) => {
                NodeOutput::plain(finish::tonemap(graph, image, tonemap))
            }
            Operation::Lens(lens) => NodeOutput::plain(finish::lens(graph, stage, image, lens)),
            Operation::Effect(_) => match plan.effect(planned) {
                None => {
                    let passthrough = planned
                        .passthrough
                        .expect("disabled effects pass an input through");
                    NodeOutput::plain(links.image(graph, outputs, passthrough, reading))
                }
                Some(effect) => {
                    let program = &effect.program;
                    let mut inputs = Sockets::new();
                    for &(name, _) in &program.inputs {
                        inputs.push(links.socket(graph, outputs, node, name, reading));
                    }
                    let emission = match program.reads(Source::Emission) {
                        true => links.layers.emission(graph, scene, reading),
                        false => None,
                    };
                    let effect_node = EffectNode {
                        effect,
                        key: node.key,
                        inputs,
                        emission,
                    };
                    NodeOutput::plain(declare_effect(
                        graph,
                        stage,
                        scene,
                        &mut links.layers,
                        effect_node,
                    ))
                }
            },
        };
        outputs[index] = Some(output);
    }
    links.image(graph, outputs, composite.output, None)
}
