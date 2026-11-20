use std::f64::consts::PI;

const KERNEL_SIZE: usize = 32;
const KERNEL_OFFSET_COUNT: usize = 32;
const KERNEL_STORAGE: usize = KERNEL_SIZE * (KERNEL_OFFSET_COUNT + 1);

#[derive(Clone, Debug)]
pub(crate) struct Resampler {
    io_ratio: f64,
    request_frames: usize,
    destination_frames: usize,
    kernel: Vec<f32>,
    input: Vec<f32>,
    virtual_source_index: f64,
    primed: bool,
    first_pass: bool,
    r0: usize,
    r3: usize,
    block_size: usize,
}

impl Resampler {
    pub(crate) fn new(source_frames: usize, destination_frames: usize) -> Resampler {
        let io_ratio = source_frames as f64 / destination_frames as f64;
        let mut resampler = Resampler {
            io_ratio,
            request_frames: source_frames,
            destination_frames,
            kernel: kernel(io_ratio),
            input: vec![0.0; source_frames + KERNEL_SIZE],
            virtual_source_index: 0.0,
            primed: false,
            first_pass: true,
            r0: 0,
            r3: 0,
            block_size: 0,
        };
        resampler.update_regions(false);
        resampler
    }

    pub(crate) fn resample(&mut self, source: &[f32], destination: &mut [f32]) {
        if self.first_pass {
            let chunk = (self.block_size as f64 / self.io_ratio) as usize;
            self.run(chunk, destination, source);
        }
        self.run(self.destination_frames, destination, source);
    }

    fn run(&mut self, frames: usize, destination: &mut [f32], source: &[f32]) {
        let mut remaining = frames;
        let mut written = 0;
        if !self.primed && remaining > 0 {
            self.read(source);
            self.primed = true;
        }
        while remaining > 0 {
            let steps = ((self.block_size as f64 - self.virtual_source_index) / self.io_ratio)
                .ceil() as i64;
            for _ in 0..steps.max(0) {
                let source_index = self.virtual_source_index as usize;
                let remainder = self.virtual_source_index - source_index as f64;
                let virtual_offset = remainder * KERNEL_OFFSET_COUNT as f64;
                let offset = virtual_offset as usize;
                let k1 = &self.kernel[offset * KERNEL_SIZE..(offset + 1) * KERNEL_SIZE];
                let k2 = &self.kernel[(offset + 1) * KERNEL_SIZE..(offset + 2) * KERNEL_SIZE];
                let window = &self.input[source_index..source_index + KERNEL_SIZE];
                destination[written] = convolve(window, k1, k2, virtual_offset - offset as f64);
                written += 1;
                self.virtual_source_index += self.io_ratio;
                remaining -= 1;
                if remaining == 0 {
                    return;
                }
            }
            self.virtual_source_index -= self.block_size as f64;
            self.input.copy_within(self.r3..self.r3 + KERNEL_SIZE, 0);
            if self.r0 == KERNEL_SIZE / 2 {
                self.update_regions(true);
            }
            self.read(source);
        }
    }

    fn read(&mut self, source: &[f32]) {
        let region = &mut self.input[self.r0..self.r0 + self.request_frames];
        if self.first_pass {
            region.fill(0.0);
            self.first_pass = false;
        } else {
            region.copy_from_slice(source);
        }
    }

    fn update_regions(&mut self, second_load: bool) {
        self.r0 = if second_load {
            KERNEL_SIZE
        } else {
            KERNEL_SIZE / 2
        };
        self.r3 = self.r0 + self.request_frames - KERNEL_SIZE;
        let r4 = self.r0 + self.request_frames - KERNEL_SIZE / 2;
        self.block_size = r4 - KERNEL_SIZE / 2;
    }
}

fn kernel(io_ratio: f64) -> Vec<f32> {
    const ALPHA: f64 = 0.16;
    const A0: f64 = 0.5 * (1.0 - ALPHA);
    const A1: f64 = 0.5;
    const A2: f64 = 0.5 * ALPHA;
    let sinc_scale = (if io_ratio > 1.0 { 1.0 / io_ratio } else { 1.0 }) * 0.9;
    let mut kernel = vec![0.0f32; KERNEL_STORAGE];
    for offset_index in 0..=KERNEL_OFFSET_COUNT {
        let subsample_offset = offset_index as f32 / KERNEL_OFFSET_COUNT as f32;
        for i in 0..KERNEL_SIZE {
            let pre_sinc = (PI
                * f64::from((i as i32 - (KERNEL_SIZE / 2) as i32) as f32 - subsample_offset))
                as f32;
            let x = f64::from((i as f32 - subsample_offset) / KERNEL_SIZE as f32);
            let window = (A0 - A1 * (2.0 * PI * x).cos() + A2 * (4.0 * PI * x).cos()) as f32;
            let sinc = if pre_sinc == 0.0 {
                sinc_scale
            } else {
                (sinc_scale * f64::from(pre_sinc)).sin() / f64::from(pre_sinc)
            };
            kernel[i + offset_index * KERNEL_SIZE] = (f64::from(window) * sinc) as f32;
        }
    }
    kernel
}

fn convolve(input: &[f32], k1: &[f32], k2: &[f32], interpolation: f64) -> f32 {
    let mut sum1 = 0.0f32;
    let mut sum2 = 0.0f32;
    for ((x, a), b) in input.iter().zip(k1).zip(k2) {
        sum1 += x * a;
        sum2 += x * b;
    }
    ((1.0 - interpolation) * f64::from(sum1) + interpolation * f64::from(sum2)) as f32
}
