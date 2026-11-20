mod comfort_noise;
mod moving_average;
mod nearend_detector;
mod residual_echo_estimator;
mod suppression_filter;
mod suppression_gain;

pub(crate) use comfort_noise::ComfortNoiseGenerator;
pub(crate) use residual_echo_estimator::ResidualEchoEstimator;
pub(crate) use suppression_filter::SuppressionFilter;
pub(crate) use suppression_gain::{GainInput, SuppressionGain};
