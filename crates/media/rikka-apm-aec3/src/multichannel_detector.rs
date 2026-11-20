use crate::framing::FrameBands;

const FRAMES_PER_SECOND: i32 = 100;

#[derive(Clone, Debug)]
pub(crate) struct MultichannelContentDetector {
    detect_stereo_content: bool,
    detection_threshold: f32,
    detection_timeout_threshold_frames: Option<i32>,
    stereo_detection_hysteresis_frames: i32,
    persistent_multichannel_content_detected: bool,
    temporary_multichannel_content_detected: bool,
    frames_since_stereo_detected_last: i64,
    consecutive_frames_with_stereo: i64,
}

impl MultichannelContentDetector {
    pub(crate) fn new(
        config: &crate::config::MultiChannel,
        render_channels: usize,
    ) -> MultichannelContentDetector {
        let timeout = config.stereo_detection_timeout_threshold_seconds;
        MultichannelContentDetector {
            detect_stereo_content: config.detect_stereo_content,
            detection_threshold: config.stereo_detection_threshold,
            detection_timeout_threshold_frames: (timeout > 0).then(|| timeout * FRAMES_PER_SECOND),
            stereo_detection_hysteresis_frames: (config.stereo_detection_hysteresis_seconds
                * FRAMES_PER_SECOND as f32) as i32,
            persistent_multichannel_content_detected: !config.detect_stereo_content
                && render_channels > 1,
            temporary_multichannel_content_detected: false,
            frames_since_stereo_detected_last: 0,
            consecutive_frames_with_stereo: 0,
        }
    }

    pub(crate) fn is_proper_multichannel_content_detected(&self) -> bool {
        self.persistent_multichannel_content_detected
    }

    pub(crate) fn is_temporary_multichannel_content_detected(&self) -> bool {
        self.temporary_multichannel_content_detected
    }

    pub(crate) fn update_detection(&mut self, frame: &[FrameBands]) -> bool {
        if !self.detect_stereo_content {
            return false;
        }
        let previous = self.persistent_multichannel_content_detected;
        let stereo_detected_in_frame = has_stereo_content(frame, self.detection_threshold);
        self.consecutive_frames_with_stereo = if stereo_detected_in_frame {
            self.consecutive_frames_with_stereo + 1
        } else {
            0
        };
        self.frames_since_stereo_detected_last = if stereo_detected_in_frame {
            0
        } else {
            self.frames_since_stereo_detected_last + 1
        };
        if self.consecutive_frames_with_stereo > i64::from(self.stereo_detection_hysteresis_frames)
        {
            self.persistent_multichannel_content_detected = true;
        }
        if let Some(timeout) = self.detection_timeout_threshold_frames
            && self.frames_since_stereo_detected_last >= i64::from(timeout)
        {
            self.persistent_multichannel_content_detected = false;
        }
        self.temporary_multichannel_content_detected =
            !self.persistent_multichannel_content_detected && stereo_detected_in_frame;
        previous != self.persistent_multichannel_content_detected
    }
}

fn has_stereo_content(frame: &[FrameBands], threshold: f32) -> bool {
    if frame.len() < 2 {
        return false;
    }
    frame[0].iter().zip(&frame[1]).any(|(left, right)| {
        left.iter()
            .zip(right)
            .any(|(l, r)| (l - r).abs() > threshold)
    })
}
