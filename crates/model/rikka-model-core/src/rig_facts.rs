use crate::humanoid::Humanoid;
use crate::node::NodeId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follow {
    pub node: NodeId,
    pub source: NodeId,
    pub ratio: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reach {
    pub target: NodeId,
    pub links: Vec<NodeId>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RigFacts {
    pub humanoid: Option<Humanoid>,
    pub reaches: Vec<Reach>,
    pub follows: Vec<Follow>,
    pub twisting: Vec<NodeId>,
    pub swinging: Vec<NodeId>,
}

impl RigFacts {
    pub(crate) fn retain_nodes(&mut self, count: usize) {
        let in_range = |node: &NodeId| node.index() < count;
        if let Some(humanoid) = &mut self.humanoid {
            for (bone, node) in humanoid.clone().bones() {
                if !in_range(&node) {
                    humanoid.set_bone(bone, None);
                }
            }
        }
        self.reaches
            .retain(|reach| in_range(&reach.target) && reach.links.iter().all(in_range));
        self.follows.retain(|follow| {
            in_range(&follow.node) && in_range(&follow.source) && follow.node != follow.source
        });
        self.twisting.retain(in_range);
        self.swinging.retain(in_range);
    }
}
