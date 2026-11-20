use std::collections::VecDeque;

use rikka_coded::Coded;

use crate::packet;

#[derive(Debug, Default)]
pub(crate) struct Track {
    packets: VecDeque<Coded>,
    byte_size: usize,
}

impl Track {
    pub(crate) fn packets(&self) -> &VecDeque<Coded> {
        &self.packets
    }

    pub(crate) fn byte_size(&self) -> usize {
        self.byte_size
    }

    pub(crate) fn first_pts(&self) -> Option<i64> {
        self.packets.front().map(|packet| packet.pts_us)
    }

    pub(crate) fn last_end(&self) -> Option<i64> {
        self.packets.back().map(Coded::end_us)
    }

    pub(crate) fn push(&mut self, packet: Coded) {
        self.byte_size += packet::byte_size(&packet);
        self.packets.push_back(packet);
    }

    pub(crate) fn push_picture(&mut self, packet: Coded) {
        if self.packets.is_empty() && !packet.keyframe {
            return;
        }
        self.push(packet);
    }

    pub(crate) fn drop_front(&mut self, count: usize) -> usize {
        let mut freed = 0;
        for packet in self.packets.drain(..count.min(self.packets.len())) {
            freed += packet::byte_size(&packet);
        }
        self.byte_size -= freed;
        freed
    }

    pub(crate) fn clear(&mut self) -> usize {
        self.drop_front(self.packets.len())
    }

    pub(crate) fn keyframe_at_or_before(&self, at_us: i64) -> Option<usize> {
        self.packets
            .iter()
            .rposition(|packet| packet.keyframe && packet.pts_us <= at_us)
    }

    pub(crate) fn second_keyframe(&self) -> Option<usize> {
        self.packets
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, packet)| packet.keyframe)
            .map(|(index, _)| index)
    }

    pub(crate) fn drop_before_pts(&mut self, floor_us: i64) -> usize {
        let count = self
            .packets
            .iter()
            .take_while(|packet| packet.end_us() <= floor_us)
            .count();
        self.drop_front(count)
    }
}
