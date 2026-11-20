use std::sync::atomic::Ordering;

use libspa::pod::Pod;
use libspa::utils::Direction;
use pipewire::core::CoreRc;
use pipewire::properties::properties;
use pipewire::stream::{StreamFlags, StreamListener, StreamRc};

use crate::node::{Node, NodeKind};
use crate::peaks::Level;

pub(crate) const METER: &str = "rikka-pipewire-meter";

pub(crate) struct Meter {
    _stream: StreamRc,
    _listener: StreamListener<Level>,
}

impl Meter {
    pub(crate) fn watch(core: &CoreRc, node: &Node, level: Level) -> Option<Self> {
        let mut props = properties! {
            *pipewire::keys::MEDIA_TYPE => "Audio",
            *pipewire::keys::MEDIA_CATEGORY => "Capture",
            *pipewire::keys::NODE_NAME => METER,
            *pipewire::keys::NODE_PASSIVE => "true",
            *pipewire::keys::STREAM_MONITOR => "true",
            *pipewire::keys::TARGET_OBJECT => node.serial.to_string(),
        };
        if node.kind == NodeKind::Sink {
            props.insert(*pipewire::keys::STREAM_CAPTURE_SINK, "true");
        }

        let stream = StreamRc::new(core.clone(), METER, props).ok()?;
        let listener = stream
            .add_local_listener_with_user_data(level)
            .process(|stream, level| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let peak = buffer.datas_mut().first_mut().map_or(0.0, loudest);
                level.store(peak.to_bits(), Ordering::Relaxed);
            })
            .register()
            .ok()?;

        let pod = rikka_pipewire::float_format(None, None);
        let mut params = [Pod::from_bytes(&pod)?];

        stream
            .connect(
                Direction::Input,
                None,
                StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
                &mut params,
            )
            .ok()?;

        Some(Self {
            _stream: stream,
            _listener: listener,
        })
    }
}

fn loudest(data: &mut libspa::buffer::Data) -> f32 {
    let filled = data.chunk().size() as usize;
    let Some(samples) = data.data() else {
        return 0.0;
    };
    let filled = filled.min(samples.len());
    samples[..filled]
        .as_chunks::<{ size_of::<f32>() }>()
        .0
        .iter()
        .map(|sample| f32::from_le_bytes(*sample).abs())
        .fold(0.0, f32::max)
        .min(1.0)
}

impl std::fmt::Debug for Meter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Meter").finish_non_exhaustive()
    }
}
