use crate::common::{FFT_SIZE, FRAME_SIZE, OVERLAP};

#[expect(
    clippy::excessive_precision,
    clippy::approx_constant,
    reason = "values copied verbatim from WebRTC"
)]
const WINDOW_HALF: [f32; 96] = [
    0.00000000, 0.01636173, 0.03271908, 0.04906767, 0.06540313, 0.08172107, 0.09801714, 0.11428696,
    0.13052619, 0.14673047, 0.16289547, 0.17901686, 0.19509032, 0.21111155, 0.22707626, 0.24298018,
    0.25881905, 0.27458862, 0.29028468, 0.30590302, 0.32143947, 0.33688985, 0.35225005, 0.36751594,
    0.38268343, 0.39774847, 0.41270703, 0.42755509, 0.44228869, 0.45690388, 0.47139674, 0.48576339,
    0.50000000, 0.51410274, 0.52806785, 0.54189158, 0.55557023, 0.56910015, 0.58247770, 0.59569930,
    0.60876143, 0.62166057, 0.63439328, 0.64695615, 0.65934582, 0.67155895, 0.68359230, 0.69544264,
    0.70710678, 0.71858162, 0.72986407, 0.74095113, 0.75183981, 0.76252720, 0.77301045, 0.78328675,
    0.79335334, 0.80320753, 0.81284668, 0.82226822, 0.83146961, 0.84044840, 0.84920218, 0.85772861,
    0.86602540, 0.87409034, 0.88192126, 0.88951608, 0.89687274, 0.90398929, 0.91086382, 0.91749450,
    0.92387953, 0.93001722, 0.93590593, 0.94154407, 0.94693013, 0.95206268, 0.95694034, 0.96156180,
    0.96592583, 0.97003125, 0.97387698, 0.97746197, 0.98078528, 0.98384601, 0.98664333, 0.98917651,
    0.99144486, 0.99344778, 0.99518473, 0.99665524, 0.99785892, 0.99879546, 0.99946459, 0.99986614,
];

pub(crate) fn apply_window(x: &mut [f32; FFT_SIZE]) {
    for (sample, &weight) in x.iter_mut().zip(&WINDOW_HALF) {
        *sample *= weight;
    }
    for (sample, &weight) in x[161..].iter_mut().zip(WINDOW_HALF[1..].iter().rev()) {
        *sample *= weight;
    }
}

pub(crate) fn form_extended_frame(
    frame: &[f32; FRAME_SIZE],
    old_data: &mut [f32; OVERLAP],
    extended_frame: &mut [f32; FFT_SIZE],
) {
    extended_frame[..OVERLAP].copy_from_slice(old_data);
    extended_frame[OVERLAP..].copy_from_slice(frame);
    old_data.copy_from_slice(&extended_frame[FFT_SIZE - OVERLAP..]);
}

pub(crate) fn overlap_and_add(
    extended_frame: &[f32; FFT_SIZE],
    overlap_memory: &mut [f32; OVERLAP],
    output: &mut [f32; FRAME_SIZE],
) {
    for i in 0..OVERLAP {
        output[i] = overlap_memory[i] + extended_frame[i];
    }
    output[OVERLAP..].copy_from_slice(&extended_frame[OVERLAP..FRAME_SIZE]);
    overlap_memory.copy_from_slice(&extended_frame[FRAME_SIZE..]);
}

pub(crate) fn delay_signal(
    frame: &[f32; FRAME_SIZE],
    delay_buffer: &mut [f32; OVERLAP],
    delayed: &mut [f32; FRAME_SIZE],
) {
    const FROM_FRAME: usize = FRAME_SIZE - OVERLAP;
    delayed[..OVERLAP].copy_from_slice(delay_buffer);
    delayed[OVERLAP..].copy_from_slice(&frame[..FROM_FRAME]);
    delay_buffer.copy_from_slice(&frame[FROM_FRAME..]);
}

pub(crate) fn energy(x: &[f32]) -> f32 {
    let mut energy = 0.0f32;
    for &sample in x {
        energy += sample * sample;
    }
    energy
}

pub(crate) fn energy_of_parts(frame: &[f32; FRAME_SIZE], old_data: &[f32; OVERLAP]) -> f32 {
    let mut energy = 0.0f32;
    for &sample in old_data.iter().chain(frame) {
        energy += sample * sample;
    }
    energy
}
