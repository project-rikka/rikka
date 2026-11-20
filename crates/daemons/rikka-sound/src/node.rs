use crate::format::Format;
use crate::hardware::Hardware;
use crate::volume::Volume;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Sink,
    Source,
    Playback,
    Recording,
}

impl NodeKind {
    pub fn from_media_class(class: &str) -> Option<Self> {
        match class {
            "Audio/Sink" => Some(Self::Sink),
            "Audio/Source" | "Audio/Source/Virtual" => Some(Self::Source),
            "Stream/Output/Audio" => Some(Self::Playback),
            "Stream/Input/Audio" => Some(Self::Recording),
            _ => None,
        }
    }

    pub fn is_stream(self) -> bool {
        matches!(self, Self::Playback | Self::Recording)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: u32,
    pub serial: u64,
    pub kind: NodeKind,
    pub name: String,
    pub title: String,
    pub detail: Option<String>,
    pub icon: Option<String>,
    pub device: Option<u32>,
    pub route: Option<i32>,
    pub volume: Volume,
    pub format: Option<Format>,
    pub quantum: Option<(u32, u32)>,
    pub target: Option<String>,
    pub latency_offset: i64,
    pub running: bool,
    pub hardware: Option<Hardware>,
}

impl Node {
    pub fn latency(&self) -> Option<f64> {
        let (quantum, rate) = self.quantum?;
        crate::format::latency(quantum, rate)
    }
}
