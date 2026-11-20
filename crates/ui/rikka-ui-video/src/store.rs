use rikka_picture::consume::{VideoCommand, VideoConsumer, announce_arrival};
use rikka_picture::{VideoController, VideoState, WeakVideoController};
use rikka_renderer::{TextureId, Uploads};
use rikka_types::Size;
use rikka_ui::SourceKey;
use rikka_ui_host::{VideoFrame, VideoPlayback};
use rikka_video_play::{DecodeBackend, Opening, PlaybackState, VideoPlayer};
use rustc_hash::FxHashMap;

#[derive(Debug, Default)]
pub struct VideoStore {
    players: FxHashMap<u64, Slot>,
    pending: FxHashMap<u64, WeakVideoController>,
    opening: FxHashMap<u64, Preparing>,
}

#[derive(Debug)]
struct Slot {
    handle: WeakVideoController,
    player: VideoPlayer,
}

#[derive(Debug)]
struct Preparing {
    handle: WeakVideoController,
    opening: Opening,
}

impl VideoPlayback for VideoStore {
    fn advance(&mut self, uploads: &Uploads) -> VideoFrame {
        let mut frame = VideoFrame::default();
        self.start_pending(uploads);
        self.take_ready(uploads, &mut frame.settled);
        let mut closing = Vec::new();
        for (id, slot) in &mut self.players {
            let Some(controller) = slot.handle.upgrade() else {
                closing.push(*id);
                continue;
            };
            for command in controller.take_commands() {
                apply(&mut slot.player, command);
            }
            match slot.player.advance(uploads) {
                Ok(true) => frame.updated.push(slot.player.texture()),
                Ok(false) => {}
                Err(error) => {
                    controller.fail(error.to_string());
                    closing.push(*id);
                    continue;
                }
            }
            let state = slot.player.state();
            controller.set_position(slot.player.position());
            controller.set_state(match state {
                PlaybackState::Playing => VideoState::Playing,
                PlaybackState::Paused => VideoState::Paused,
                PlaybackState::Ended => VideoState::Ended,
            });
            frame.due = [frame.due, slot.player.next_due()]
                .into_iter()
                .flatten()
                .min();
        }

        for id in closing {
            if let Some(slot) = self.players.remove(&id) {
                slot.player.close(uploads);
            }
        }
        frame
    }

    fn is_starting(&self) -> bool {
        !self.pending.is_empty()
    }

    fn has_work(&self) -> bool {
        self.is_starting() || !self.opening.is_empty() || !self.players.is_empty()
    }

    fn resolve(&mut self, controller: &VideoController) -> Option<TextureId> {
        if let Some(slot) = self.players.get(&controller.id()) {
            return Some(slot.player.texture());
        }
        if controller.state() != VideoState::Failed && !self.opening.contains_key(&controller.id())
        {
            self.pending
                .entry(controller.id())
                .or_insert_with(|| controller.downgrade());
        }
        None
    }
}

impl VideoStore {
    fn start_pending(&mut self, uploads: &Uploads) {
        if self.pending.is_empty() {
            return;
        }
        let device = uploads.video_device();
        for (id, handle) in std::mem::take(&mut self.pending) {
            let Some(controller) = handle.upgrade() else {
                continue;
            };
            let opening = VideoPlayer::opening(
                device.clone(),
                controller.path().to_path_buf(),
                DecodeBackend::Auto,
                announce_arrival,
            );
            self.opening.insert(id, Preparing { handle, opening });
        }
    }

    fn take_ready(&mut self, uploads: &Uploads, newly_ready: &mut Vec<SourceKey>) {
        let mut answered = Vec::new();
        self.opening.retain(|id, preparing| {
            if preparing.handle.is_dropped() {
                return false;
            }
            match preparing.opening.ready() {
                Some(answer) => {
                    answered.push((*id, preparing.handle.clone(), answer));
                    false
                }
                None => true,
            }
        });
        for (id, handle, answer) in answered {
            let Some(controller) = handle.upgrade() else {
                continue;
            };
            match answer.and_then(|ready| ready.play_on(uploads)) {
                Ok(player) => {
                    let info = player.info();
                    controller.set_duration(info.duration);
                    controller.set_size(Size::new(info.width as f32, info.height as f32));
                    controller.set_state(VideoState::Paused);
                    self.players.insert(id, Slot { handle, player });
                    newly_ready.push(SourceKey::Video(id));
                }
                Err(error) => controller.fail(error.to_string()),
            }
        }
    }
}

impl Drop for VideoStore {
    fn drop(&mut self) {
        self.players.clear();
        self.opening.clear();
        rikka_video_play::wait_for_closed_players();
    }
}

fn apply(player: &mut VideoPlayer, command: VideoCommand) {
    match command {
        VideoCommand::Play => player.play(),
        VideoCommand::Pause => player.pause(),
        VideoCommand::Seek(position) => player.seek(position),
        VideoCommand::SetLooping(looping) => player.set_looping(looping),
        VideoCommand::SetSpeed(speed) => player.set_speed(speed),
    }
}
