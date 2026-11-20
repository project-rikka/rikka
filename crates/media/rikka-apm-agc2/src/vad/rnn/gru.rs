use super::dense::scaled;
use crate::vad::common::dot;

const GATES: usize = 3;
const MAX_UNITS: usize = 24;

#[derive(Clone, Debug)]
pub(crate) struct Gru {
    input_size: usize,
    output_size: usize,
    bias: Vec<f32>,
    weights: Vec<f32>,
    recurrent_weights: Vec<f32>,
    state: Vec<f32>,
}

impl Gru {
    pub(crate) fn new(
        input_size: usize,
        bias: &[i8],
        weights: &[i8],
        recurrent_weights: &[i8],
    ) -> Gru {
        let output_size = bias.len() / GATES;
        Gru {
            input_size,
            output_size,
            bias: by_gate(bias, output_size),
            weights: by_gate(weights, output_size),
            recurrent_weights: by_gate(recurrent_weights, output_size),
            state: vec![0.0; output_size],
        }
    }

    pub(crate) fn state(&self) -> &[f32] {
        &self.state
    }

    pub(crate) fn reset(&mut self) {
        self.state.fill(0.0);
    }

    pub(crate) fn compute(&mut self, input: &[f32]) {
        let update = self.gate(0, input);
        let reset = self.gate(1, input);
        let mut reset_state = [0.0f32; MAX_UNITS];
        for o in 0..self.output_size {
            reset_state[o] = self.state[o] * reset[o];
        }
        for o in 0..self.output_size {
            let x = self.bias[2 * self.output_size + o]
                + dot(input, self.weights(2, o))
                + dot(
                    &reset_state[..self.output_size],
                    self.recurrent_weights(2, o),
                );
            self.state[o] = update[o] * self.state[o] + (1.0 - update[o]) * x.max(0.0);
        }
    }

    fn gate(&self, gate: usize, input: &[f32]) -> [f32; MAX_UNITS] {
        let mut values = [0.0f32; MAX_UNITS];
        for (o, value) in values[..self.output_size].iter_mut().enumerate() {
            let x = self.bias[gate * self.output_size + o]
                + dot(input, self.weights(gate, o))
                + dot(&self.state, self.recurrent_weights(gate, o));
            *value = super::activations::sigmoid(x);
        }
        values
    }

    fn weights(&self, gate: usize, o: usize) -> &[f32] {
        let start = gate * self.input_size * self.output_size + o * self.input_size;
        &self.weights[start..start + self.input_size]
    }

    fn recurrent_weights(&self, gate: usize, o: usize) -> &[f32] {
        let start = gate * self.output_size * self.output_size + o * self.output_size;
        &self.recurrent_weights[start..start + self.output_size]
    }
}

fn by_gate(tensor: &[i8], output_size: usize) -> Vec<f32> {
    let n = tensor.len() / (output_size * GATES);
    let stride_src = GATES * output_size;
    let stride_dst = n * output_size;
    let mut result = vec![0.0; tensor.len()];
    for g in 0..GATES {
        for o in 0..output_size {
            for i in 0..n {
                result[g * stride_dst + o * n + i] =
                    scaled(tensor[i * stride_src + g * output_size + o]);
            }
        }
    }
    result
}
