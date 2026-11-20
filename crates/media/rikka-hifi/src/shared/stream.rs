use std::cell::RefCell;
use std::rc::Rc;

use libspa::pod::Pod;
use libspa::sys;
use libspa::utils::Direction;
use pipewire::core::CoreRc;
use pipewire::keys;
use pipewire::properties::properties;
use pipewire::stream::{Stream, StreamFlags, StreamListener, StreamRc, StreamState};

use crate::notice::{Notice, Notifier};
use crate::ring::RingReader;
use crate::shared::Setup;
use crate::shared::graph::{Graph, with};

pub(super) struct Playing {
    stream: StreamRc,
    _listener: StreamListener<()>,
    channels: usize,
}

impl Playing {
    pub(super) fn connect(
        core: &CoreRc,
        setup: &Setup,
        ring: RingReader,
        graph: &Rc<RefCell<Graph>>,
        notifier: Notifier,
    ) -> Result<Playing, pipewire::Error> {
        let plan = setup.plan;
        let identity = &setup.identity;
        let mut props = properties! {
            *keys::MEDIA_TYPE => "Audio",
            *keys::MEDIA_CATEGORY => "Playback",
            *keys::MEDIA_ROLE => "Music",
            *keys::APP_NAME => identity.name.clone(),
            *keys::APP_ID => identity.id.clone(),
            *keys::NODE_NAME => identity.id.clone(),
            *keys::NODE_DESCRIPTION => identity.name.clone(),
            *keys::NODE_RATE => format!("1/{}", plan.rate),
        };
        if let Some(icon) = &identity.icon {
            props.insert(*keys::APP_ICON_NAME, icon.clone());
        }
        if let Some(sink) = &setup.sink {
            props.insert(*keys::TARGET_OBJECT, sink.clone());
        }
        let stream = StreamRc::new(core.clone(), &identity.name, props)?;

        let mut filler = Filler {
            ring,
            frame: plan.frame_bytes(),
            rate: plan.rate,
            silence: 0,
        };
        let watched = Rc::downgrade(graph);
        let listener = stream
            .add_local_listener_with_user_data(())
            .state_changed(move |stream, (), _, state| match state {
                StreamState::Paused | StreamState::Streaming => {
                    let node = stream.node_id();
                    with(&watched, |graph| graph.streaming_as(node, &watched));
                }
                StreamState::Error(message) => notifier.send(Notice::SharedFailed(message)),
                StreamState::Unconnected | StreamState::Connecting => {}
            })
            .process(move |stream, ()| filler.fill(stream))
            .register()?;

        let format = rikka_pipewire::float_format(Some(plan.rate), Some(plan.channels));
        let format = Pod::from_bytes(&format).ok_or(pipewire::Error::CreationFailed)?;
        stream.connect(
            Direction::Output,
            None,
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
            &mut [format],
        )?;
        let playing = Playing {
            stream,
            _listener: listener,
            channels: usize::from(plan.channels),
        };
        playing.set_volume(setup.volume);
        Ok(playing)
    }

    pub(super) fn set_active(&self, active: bool) {
        if let Err(error) = self.stream.set_active(active) {
            tracing::debug!("failed to set hifi stream active state: {error}");
        }
    }

    pub(super) fn set_volume(&self, volume: f32) {
        let levels = vec![volume.clamp(0.0, 1.0); self.channels];
        if let Err(error) = self
            .stream
            .set_control(sys::SPA_PROP_channelVolumes, &levels)
        {
            tracing::debug!("failed to set hifi stream volume: {error}");
        }
    }
}

struct Filler {
    ring: RingReader,
    frame: usize,
    rate: u32,
    silence: u64,
}

impl Filler {
    fn fill(&mut self, stream: &Stream) {
        let Some(mut buffer) = stream.dequeue_buffer() else {
            return;
        };
        let requested = usize::try_from(buffer.requested()).unwrap_or(usize::MAX);
        let Some(data) = buffer.datas_mut().first_mut() else {
            return;
        };
        let frame = self.frame;
        let filled = match data.data() {
            Some(bytes) => {
                let fitting = bytes.len() / frame;
                let frames = if requested == 0 {
                    fitting
                } else {
                    requested.min(fitting)
                };
                let area = &mut bytes[..frames * frame];
                let pulled = self.ring.pull(area);
                area[pulled..].fill(0);
                let silent = ((area.len() - pulled) / frame) as u64;
                self.silence = if pulled == 0 {
                    self.silence + silent
                } else {
                    silent
                };
                area.len()
            }
            None => 0,
        };
        let chunk = data.chunk_mut();
        *chunk.offset_mut() = 0;
        *chunk.stride_mut() = i32::try_from(frame).unwrap_or(i32::MAX);
        *chunk.size_mut() = u32::try_from(filled).unwrap_or(u32::MAX);
        drop(buffer);
        if let Ok(time) = stream.time() {
            let latency = self.latency(&time);
            self.ring.set_latency(latency.saturating_sub(self.silence));
        }
    }

    fn latency(&self, time: &pipewire::stream::Time) -> u64 {
        let clock = time.rate();
        let delay = u64::try_from(time.delay()).unwrap_or(0);
        let device = if clock.denom == 0 {
            0
        } else {
            delay * u64::from(clock.num) * u64::from(self.rate) / u64::from(clock.denom)
        };
        device + time.queued() / self.frame as u64 + time.buffered()
    }
}
