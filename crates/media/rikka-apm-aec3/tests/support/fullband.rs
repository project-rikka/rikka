use rikka_apm_aec3::{Config, EchoCanceller, Metrics};
use rikka_apm_audio::{FRAME, Frame};

use super::Lcg;

const ECHO_PATH: [f32; 5] = [0.6, 0.25, -0.15, 0.1, -0.05];
const REFLECTION_LAG: usize = 480;
const REFLECTION_GAIN: f32 = 0.2;

pub(crate) struct Talk {
    pub(crate) frames: usize,
    pub(crate) delay_samples: usize,
    pub(crate) near_frames: std::ops::Range<usize>,
}

pub(crate) struct Conversation {
    pub(crate) far: Vec<f32>,
    pub(crate) echo: Vec<f32>,
    pub(crate) near: Vec<f32>,
    pub(crate) capture: Vec<f32>,
}

fn syllables(n: usize, period: usize, offset: usize) -> f32 {
    let phase = (n + offset) % period;
    let voiced = period * 3 / 4;
    if phase >= voiced {
        return 0.0;
    }
    let half = voiced as f32 / 2.0;
    1.0 - (phase as f32 - half).abs() / half
}

impl Conversation {
    pub(crate) fn new(talk: &Talk) -> Conversation {
        let total = talk.frames * FRAME;
        let (mut far_noise, mut near_noise, mut floor) = (Lcg::new(11), Lcg::new(23), Lcg::new(37));
        let (mut far_colour, mut near_colour) = (0.0_f32, 0.0_f32);
        let mut far = vec![0.0; total];
        let mut echo = vec![0.0; total];
        let mut near = vec![0.0; total];
        let mut capture = vec![0.0; total];
        for n in 0..total {
            far_colour = 0.97 * far_colour + far_noise.next();
            far[n] = 2500.0 * syllables(n, 12_000, 0) * far_colour;
            near_colour = 0.9 * near_colour + near_noise.next();
            let talking = talk.near_frames.contains(&(n / FRAME));
            near[n] = if talking {
                2500.0 * syllables(n, 10_000, 3_000) * near_colour
            } else {
                0.0
            };
            let tap = |lag: usize| n.checked_sub(lag).map_or(0.0, |i| far[i]);
            let direct: f32 = ECHO_PATH
                .iter()
                .enumerate()
                .map(|(k, h)| h * tap(talk.delay_samples + k))
                .sum();
            echo[n] = direct + REFLECTION_GAIN * tap(talk.delay_samples + REFLECTION_LAG);
            capture[n] = echo[n] + near[n] + 5.0 * floor.next();
        }
        Conversation {
            far,
            echo,
            near,
            capture,
        }
    }
}

pub(crate) struct Processed {
    pub(crate) output: Vec<f32>,
    pub(crate) metrics: Vec<Metrics>,
}

pub(crate) fn process(conversation: &Conversation) -> Processed {
    let mut canceller = EchoCanceller::new(&Config::default(), 1, 1);
    let mut render = Frame::new(1);
    let mut capture = Frame::new(1);
    let frames = conversation.far.len() / FRAME;
    let mut processed = Processed {
        output: Vec::with_capacity(frames * FRAME),
        metrics: Vec::with_capacity(frames),
    };
    let (far, _) = conversation.far.as_chunks::<FRAME>();
    let (near, _) = conversation.capture.as_chunks::<FRAME>();
    for (far, near) in far.iter().zip(near) {
        *render.full_mut(0) = *far;
        render.split();
        canceller.render(&render);
        *capture.full_mut(0) = *near;
        canceller.analyze_capture(&capture);
        capture.split();
        canceller.process_capture(&mut capture, false);
        capture.merge();
        processed.output.extend_from_slice(capture.full(0));
        processed.metrics.push(canceller.metrics());
    }
    processed
}
