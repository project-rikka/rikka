use crate::config::DeviceTuning;
use crate::node::NodeKind;
use crate::volume::Volume;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Volume { node: u32, volume: Volume },
    Default { kind: NodeKind, name: String },
    Target { node: u32, sink: Option<String> },
    Profile { device: u32, index: i32 },
    Route { device: u32, index: i32, port: i32 },
    LatencyOffset { node: u32, nanoseconds: i64 },
    Tune { node: u32, tuning: DeviceTuning },
    ForceRate(u32),
    ForceQuantum(u32),
    AllowRates(Vec<u32>),
    Meter(Vec<u32>),
    Stop,
}
