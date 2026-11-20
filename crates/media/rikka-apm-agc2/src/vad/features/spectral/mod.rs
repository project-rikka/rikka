mod bands;
mod cepstrum_history;
mod distance_matrix;

use std::f64::consts::PI;

use bands::OPUS_BANDS_24K;
use cepstrum_history::CepstrumHistory;
use distance_matrix::DistanceMatrix;

use crate::vad::common::{
    CEPSTRAL_HISTORY, FEATURE_VECTOR, FRAME_20MS_24K, NUM_BANDS, NUM_LOWER_BANDS,
};
use rikka_fft::RealFft;

const SILENCE_THRESHOLD: f32 = 0.04;
const HALF_WINDOW: usize = FRAME_20MS_24K / 2;

#[derive(Clone, Debug)]
pub(crate) struct SpectralFeatures {
    half_window: [f32; HALF_WINDOW],
    fft: RealFft,
    fft_input: Vec<f32>,
    reference_fft: Vec<f32>,
    lagged_fft: Vec<f32>,
    reference_energy: [f32; OPUS_BANDS_24K],
    lagged_energy: [f32; OPUS_BANDS_24K],
    cross_corr: [f32; OPUS_BANDS_24K],
    dct_table: [f32; NUM_BANDS * NUM_BANDS],
    cepstra: CepstrumHistory,
    distances: DistanceMatrix,
}

impl SpectralFeatures {
    pub(crate) fn new() -> SpectralFeatures {
        SpectralFeatures {
            half_window: half_vorbis_window(1.0 / FRAME_20MS_24K as f32),
            fft: RealFft::new(FRAME_20MS_24K),
            fft_input: vec![0.0; FRAME_20MS_24K],
            reference_fft: vec![0.0; FRAME_20MS_24K],
            lagged_fft: vec![0.0; FRAME_20MS_24K],
            reference_energy: [0.0; OPUS_BANDS_24K],
            lagged_energy: [0.0; OPUS_BANDS_24K],
            cross_corr: [0.0; OPUS_BANDS_24K],
            dct_table: bands::dct_table(),
            cepstra: CepstrumHistory::new(),
            distances: DistanceMatrix::new(),
        }
    }

    pub(crate) fn compute(
        &mut self,
        reference: &[f32],
        lagged: &[f32],
        features: &mut [f32; FEATURE_VECTOR],
    ) -> bool {
        windowed_fft(
            reference,
            &self.half_window,
            &mut self.fft_input,
            &mut self.reference_fft,
            &mut self.fft,
        );
        bands::cross_correlation(
            &self.reference_fft,
            &self.reference_fft,
            &mut self.reference_energy,
        );
        let total_energy = self.reference_energy.iter().fold(0.0f32, |sum, x| sum + x);
        if total_energy < SILENCE_THRESHOLD {
            return true;
        }
        windowed_fft(
            lagged,
            &self.half_window,
            &mut self.fft_input,
            &mut self.lagged_fft,
            &mut self.fft,
        );
        bands::cross_correlation(&self.lagged_fft, &self.lagged_fft, &mut self.lagged_energy);

        let mut log_bands_energy = [0.0f32; NUM_BANDS];
        bands::smoothed_log_magnitude(&self.reference_energy, &mut log_bands_energy);
        let mut cepstrum = [0.0f32; NUM_BANDS];
        bands::dct(&log_bands_energy, &self.dct_table, &mut cepstrum);
        cepstrum[0] -= 12.0;
        cepstrum[1] -= 4.0;
        self.cepstra.push(&cepstrum);
        self.update_distances(&cepstrum);

        features[NUM_LOWER_BANDS..NUM_BANDS].copy_from_slice(&cepstrum[NUM_LOWER_BANDS..]);
        self.average_and_derivatives(features);
        self.normalized_cepstral_correlation(
            &mut features[NUM_BANDS + 2 * NUM_LOWER_BANDS..NUM_BANDS + 3 * NUM_LOWER_BANDS],
        );
        features[FEATURE_VECTOR - 1] = self.variability();
        false
    }

    fn update_distances(&mut self, cepstrum: &[f32; NUM_BANDS]) {
        let mut distances = [0.0f32; CEPSTRAL_HISTORY - 1];
        for (index, distance) in distances.iter_mut().enumerate() {
            let old = self.cepstra.get(index + 1);
            for (new, old) in cepstrum.iter().zip(old) {
                let c = new - old;
                *distance += c * c;
            }
        }
        self.distances.push(&distances);
    }

    fn average_and_derivatives(&self, features: &mut [f32; FEATURE_VECTOR]) {
        let current = self.cepstra.get(0);
        let previous1 = self.cepstra.get(1);
        let previous2 = self.cepstra.get(2);
        for i in 0..NUM_LOWER_BANDS {
            features[i] = current[i] + previous1[i] + previous2[i];
            features[NUM_BANDS + i] = current[i] - previous2[i];
            features[NUM_BANDS + NUM_LOWER_BANDS + i] =
                current[i] - 2.0 * previous1[i] + previous2[i];
        }
    }

    fn normalized_cepstral_correlation(&mut self, output: &mut [f32]) {
        bands::cross_correlation(&self.reference_fft, &self.lagged_fft, &mut self.cross_corr);
        for ((value, reference), lagged) in self
            .cross_corr
            .iter_mut()
            .zip(&self.reference_energy)
            .zip(&self.lagged_energy)
        {
            *value /= (0.001 + reference * lagged).sqrt();
        }
        bands::dct(&self.cross_corr, &self.dct_table, output);
        output[0] -= 1.3;
        output[1] -= 0.9;
    }

    fn variability(&self) -> f32 {
        let mut variability = 0.0f32;
        for delay1 in 0..CEPSTRAL_HISTORY {
            let mut min_distance = f32::MAX;
            for delay2 in 0..CEPSTRAL_HISTORY {
                if delay1 != delay2 {
                    min_distance = min_distance.min(self.distances.get(delay1, delay2));
                }
            }
            variability += min_distance;
        }
        variability / CEPSTRAL_HISTORY as f32 - 2.1
    }
}

fn half_vorbis_window(scaling: f32) -> [f32; HALF_WINDOW] {
    let mut window = [0.0f32; HALF_WINDOW];
    for (i, value) in window.iter_mut().enumerate() {
        let inner = (0.5 * PI * (i as f64 + 0.5) / HALF_WINDOW as f64).sin();
        *value = (f64::from(scaling) * (0.5 * PI * inner * inner).sin()) as f32;
    }
    window
}

fn windowed_fft(
    frame: &[f32],
    half_window: &[f32; HALF_WINDOW],
    input: &mut [f32],
    output: &mut [f32],
    fft: &mut RealFft,
) {
    for (i, &weight) in half_window.iter().enumerate() {
        let j = FRAME_20MS_24K - 1 - i;
        input[i] = frame[i] * weight;
        input[j] = frame[j] * weight;
    }
    fft.forward_ordered(input, output);
    output[1] = 0.0;
}
