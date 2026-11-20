use std::collections::VecDeque;
use std::sync::Arc;

use rikka_apm_audio::FRAME;
use rikka_sync::Mutex;

const CAPACITY: usize = 100;

#[derive(Clone, Debug, Default)]
pub struct Reference {
    queue: Arc<Mutex<Queue>>,
}

#[derive(Debug, Default)]
struct Queue {
    frames: VecDeque<[f32; FRAME]>,
    partial: Vec<f32>,
}

impl Reference {
    pub fn render(&self, interleaved: &[f32], channels: usize) {
        let channels = channels.max(1);
        let mut queue = self.queue.lock();
        for frame in interleaved.chunks_exact(channels) {
            queue
                .partial
                .push(frame.iter().sum::<f32>() / channels as f32);
            if queue.partial.len() == FRAME {
                let mut whole = [0.0; FRAME];
                whole.copy_from_slice(&queue.partial);
                queue.partial.clear();
                queue.frames.push_back(whole);
                if queue.frames.len() > CAPACITY {
                    queue.frames.pop_front();
                }
            }
        }
    }

    pub(crate) fn take(&self) -> Option<[f32; FRAME]> {
        self.queue.lock().frames.pop_front()
    }

    pub(crate) fn clear(&self) {
        self.queue.lock().frames.clear();
    }
}
