use std::f32::consts::TAU;

use rikka_fft::RealFft;

const SIZE: usize = 2048;
const BIN: usize = 37;

#[test]
fn ordered_spectrum_peaks_at_the_tone() {
    let tone: Vec<f32> = (0..SIZE)
        .map(|index| (TAU * BIN as f32 * index as f32 / SIZE as f32).cos())
        .collect();
    let mut spectrum = vec![0.0; SIZE];
    RealFft::new(SIZE).forward_ordered(&tone, &mut spectrum);
    for (bin, pair) in spectrum.as_chunks::<2>().0.iter().enumerate().skip(1) {
        let magnitude = pair[0].hypot(pair[1]);
        let expected = if bin == BIN { SIZE as f32 / 2.0 } else { 0.0 };
        assert!(
            (magnitude - expected).abs() < 0.05,
            "bin {bin}: {magnitude}"
        );
    }
}

#[test]
fn backward_undoes_forward() {
    let signal: Vec<f32> = (0..SIZE)
        .map(|index| ((index * 7919) % 257) as f32 / 257.0 - 0.5)
        .collect();
    let mut fft = RealFft::new(SIZE);
    let mut spectrum = vec![0.0; SIZE];
    fft.forward(&signal, &mut spectrum);
    fft.backward(&mut spectrum);
    for (restored, original) in spectrum.iter().zip(&signal) {
        assert!((restored / SIZE as f32 - original).abs() < 1e-4);
    }
}
