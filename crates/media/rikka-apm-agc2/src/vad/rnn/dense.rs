use crate::vad::common::dot;

const WEIGHTS_SCALE: f32 = 1.0 / 256.0;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Activation {
    Tansig,
    Sigmoid,
}

#[derive(Clone, Debug)]
pub(crate) struct Dense {
    input_size: usize,
    bias: Vec<f32>,
    weights: Vec<f32>,
    activation: Activation,
    output: Vec<f32>,
}

impl Dense {
    pub(crate) fn new(
        input_size: usize,
        bias: &[i8],
        weights: &[i8],
        activation: Activation,
    ) -> Dense {
        let output_size = bias.len();
        Dense {
            input_size,
            bias: bias.iter().map(|&b| scaled(b)).collect(),
            weights: transposed(weights, input_size, output_size),
            activation,
            output: vec![0.0; output_size],
        }
    }

    pub(crate) fn output(&self) -> &[f32] {
        &self.output
    }

    pub(crate) fn compute(&mut self, input: &[f32]) {
        for (o, value) in self.output.iter_mut().enumerate() {
            let weights = &self.weights[o * self.input_size..(o + 1) * self.input_size];
            let x = self.bias[o] + dot(input, weights);
            *value = match self.activation {
                Activation::Tansig => super::activations::tansig(x),
                Activation::Sigmoid => super::activations::sigmoid(x),
            };
        }
    }
}

pub(crate) fn scaled(weight: i8) -> f32 {
    WEIGHTS_SCALE * f32::from(weight)
}

fn transposed(weights: &[i8], input_size: usize, output_size: usize) -> Vec<f32> {
    let mut result = vec![0.0; weights.len()];
    for o in 0..output_size {
        for i in 0..input_size {
            result[o * input_size + i] = scaled(weights[i * output_size + o]);
        }
    }
    result
}
