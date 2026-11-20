use rikka_apm_aec3::{Config, EchoCanceller, Metrics};
use rikka_apm_audio::{BAND, BANDS, Frame};

use super::Lcg;

const ECHO: [f32; 6] = [0.4, -0.25, 0.15, 0.1, -0.05, 0.02];
const RENDER_GAIN: [f32; 2] = [1.0, 0.5];

#[derive(Clone, Copy, Debug)]
pub(crate) enum Variant {
    Default,
    SignalDependentErle,
    Tuned,
    ExternalDelay,
}

#[derive(Clone, Debug)]
pub(crate) struct Scenario {
    pub(crate) frames: usize,
    pub(crate) render_channels: usize,
    pub(crate) capture_channels: usize,
    pub(crate) variant: Variant,
    pub(crate) delay_change_frame: Option<usize>,
    pub(crate) jitter: bool,
    pub(crate) saturation: std::ops::Range<usize>,
    pub(crate) echo_path_change_frame: Option<usize>,
    pub(crate) buffer_delay_ms: Option<i32>,
    pub(crate) near_frames: std::ops::Range<usize>,
}

impl Scenario {
    pub(crate) fn mono(frames: usize) -> Scenario {
        Scenario {
            frames,
            render_channels: 1,
            capture_channels: 1,
            variant: Variant::Default,
            delay_change_frame: None,
            jitter: false,
            saturation: 0..0,
            echo_path_change_frame: None,
            buffer_delay_ms: None,
            near_frames: 0..0,
        }
    }
}

pub(crate) struct Signals {
    far: Vec<[Vec<f32>; BANDS]>,
    capture: Vec<[Vec<f32>; BANDS]>,
}

impl Signals {
    pub(crate) fn new(scenario: &Scenario) -> Signals {
        let total = scenario.frames * BAND;
        let bands = || -> [Vec<f32>; BANDS] { std::array::from_fn(|_| vec![0.0; total]) };
        let mut far: Vec<_> = (0..scenario.render_channels).map(|_| bands()).collect();
        let mut capture: Vec<_> = (0..scenario.capture_channels).map(|_| bands()).collect();
        let mut far_lcg: Vec<[Lcg; BANDS]> = (0..2)
            .map(|r| std::array::from_fn(|b| Lcg::new(1 + 10 * r + b as u32)))
            .collect();
        let mut near_lcg = [Lcg::new(4), Lcg::new(14)];
        let mut noise_lcg: Vec<[Lcg; BANDS]> = (0..2)
            .map(|c| std::array::from_fn(|b| Lcg::new(5 + 10 * c + b as u32)))
            .collect();
        let mut colour = [0.0_f32; 2];
        let mut near_colour = [0.0_f32; 2];
        for n in 0..total {
            let frame = n / BAND;
            let env = if (n / 1600) % 5 != 4 { 1.0 } else { 0.05 };
            for (r, far) in far.iter_mut().enumerate() {
                colour[r] = 0.95 * colour[r] + 0.3 * far_lcg[r][0].next();
                far[0][n] = 6000.0 * env * colour[r];
                far[1][n] = 2000.0 * env * far_lcg[r][1].next();
                far[2][n] = 2000.0 * env * far_lcg[r][2].next();
            }
            let delay = if scenario
                .delay_change_frame
                .is_some_and(|start| frame >= start)
            {
                960
            } else {
                480
            };
            let gate = if scenario.near_frames.contains(&frame) {
                1.0
            } else {
                0.0
            };
            for c in 0..scenario.capture_channels {
                near_colour[c] = 0.8 * near_colour[c] + 0.4 * near_lcg[c].next();
                let near = 3000.0 * gate * near_colour[c];
                let mut echo = 0.0_f32;
                for (r, far) in far.iter().enumerate() {
                    for (k, tap) in ECHO.iter().enumerate() {
                        let x = n
                            .checked_sub(delay + 8 * c + 40 * r + k)
                            .map_or(0.0, |i| far[0][i]);
                        echo += tap * RENDER_GAIN[r] * x;
                    }
                }
                let delayed = |band: usize| n.checked_sub(delay).map_or(0.0, |i| far[0][band][i]);
                capture[c][0][n] = echo + near + 30.0 * noise_lcg[c][0].next();
                capture[c][1][n] = 0.2 * delayed(1) + 0.3 * near + 10.0 * noise_lcg[c][1].next();
                capture[c][2][n] = 0.2 * delayed(2) + 10.0 * noise_lcg[c][2].next();
            }
        }
        Signals { far, capture }
    }
}

fn load(signal: &[[Vec<f32>; BANDS]], frame_index: usize, frame: &mut Frame) {
    for (channel, bands) in signal.iter().enumerate() {
        for (band, samples) in bands.iter().enumerate() {
            frame
                .band_mut(channel, band)
                .copy_from_slice(&samples[frame_index * BAND..(frame_index + 1) * BAND]);
        }
    }
}

pub(crate) fn configs(variant: Variant) -> (Config, Config) {
    let mut config = Config::default();
    let mut multichannel = Config::multichannel();
    match variant {
        Variant::Default => {}
        Variant::SignalDependentErle => {
            config.erle.num_sections = 3;
            config.ep_strength.default_len = -0.83;
            config.echo_audibility.use_stationarity_properties = true;
            config.echo_audibility.use_stationarity_properties_at_init = true;
        }
        Variant::Tuned => {
            let suppressor = &mut config.suppressor;
            suppressor.use_subband_nearend_detection = true;
            suppressor.subband_nearend_detection.subband1.low = 1;
            suppressor.subband_nearend_detection.subband1.high = 10;
            suppressor.subband_nearend_detection.subband2.low = 11;
            suppressor.subband_nearend_detection.subband2.high = 20;
            suppressor.subband_nearend_detection.nearend_average_blocks = 2;
            suppressor.conservative_hf_suppression = true;
            config.filter.high_pass_filter_echo_reference = true;
            config.delay.down_sampling_factor = 8;
            config.delay.fixed_capture_delay_samples = 32;
            config.filter.enable_coarse_filter_output_usage = false;
            config.ep_strength.bounded_erl = true;
            multichannel.filter.high_pass_filter_echo_reference = true;
            multichannel.delay.fixed_capture_delay_samples = 32;
        }
        Variant::ExternalDelay => config.delay.use_external_delay_estimator = true,
    }
    (config, multichannel)
}

pub(crate) struct Run {
    pub(crate) output: Vec<Vec<[[f32; BAND]; BANDS]>>,
    pub(crate) metrics: Vec<Metrics>,
}

pub(crate) fn run(scenario: &Scenario, signals: &Signals) -> Run {
    let (config, multichannel) = configs(scenario.variant);
    let mut canceller = EchoCanceller::with_multichannel_config(
        &config,
        Some(&multichannel),
        scenario.render_channels,
        scenario.capture_channels,
    );
    let mut render = Frame::new(scenario.render_channels);
    let mut capture = Frame::new(scenario.capture_channels);
    let mut run = Run {
        output: Vec::with_capacity(scenario.frames),
        metrics: Vec::with_capacity(scenario.frames),
    };
    let mut pending = 0;
    for f in 0..scenario.frames {
        let mut push = |index: usize, canceller: &mut EchoCanceller| {
            load(&signals.far, index, &mut render);
            canceller.render(&render);
        };
        if scenario.jitter && f % 7 == 3 {
            pending = f;
        } else {
            if scenario.jitter && f % 7 == 4 {
                push(pending, &mut canceller);
            }
            push(f, &mut canceller);
        }
        load(&signals.capture, f, &mut capture);
        for c in 0..scenario.capture_channels {
            for (k, sample) in capture.full_mut(c).iter_mut().enumerate() {
                *sample = if scenario.saturation.contains(&f) && k < 10 {
                    32_767.0
                } else {
                    0.0
                };
            }
        }
        if let Some(delay) = scenario.buffer_delay_ms {
            canceller.set_audio_buffer_delay(delay);
        }
        canceller.analyze_capture(&capture);
        canceller.process_capture(&mut capture, scenario.echo_path_change_frame == Some(f));
        run.output.push(
            (0..scenario.capture_channels)
                .map(|c| *capture.bands(c))
                .collect(),
        );
        run.metrics.push(canceller.metrics());
    }
    run
}
