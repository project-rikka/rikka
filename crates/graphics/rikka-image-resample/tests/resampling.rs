use rikka_image_resample::{Axis, Footprint, Job, Kernel, Resampler, Threads};
use rikka_simd::Level;

const KERNELS: [Kernel; 4] = [
    Kernel::Nearest,
    Kernel::Bilinear,
    Kernel::Bicubic,
    Kernel::Lanczos3,
];

fn texture(width: usize, height: usize, channels: usize) -> Vec<f32> {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    (0..width * height * channels)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / (1u64 << 24) as f32
        })
        .collect()
}

fn run<const C: usize>(resampler: &mut Resampler, source: &[f32], job: Job) -> Vec<f32> {
    let pitch = job.x.source_len * C;
    let mut target = vec![0.0f32; job.x.target_len * job.y.target_len * C];
    resampler.resample::<f32, C>(
        job,
        &mut target,
        |row, out| out.copy_from_slice(&source[row * pitch..(row + 1) * pitch]),
        |blended, out| out.copy_from_slice(blended),
    );
    target
}

fn fitted(from: (usize, usize), to: (usize, usize), kernel: Kernel) -> Job {
    Job {
        x: Axis::fit(from.0, to.0),
        y: Axis::fit(from.1, to.1),
        footprint: Footprint::Filtered(kernel),
        threads: Threads::Calling,
    }
}

#[test]
fn flat_pictures_stay_flat_and_equal_sizes_copy() {
    let mut resampler = Resampler::new();
    let flat = vec![0.625f32; 23 * 17 * 4];
    let detailed = texture(23, 17, 4);
    for kernel in KERNELS {
        for to in [(23, 17), (7, 5), (1, 1), (40, 31), (22, 60)] {
            let out = run::<4>(&mut resampler, &flat, fitted((23, 17), to, kernel));
            assert_eq!(out.len(), to.0 * to.1 * 4);
            for value in out {
                assert!((value - 0.625).abs() < 1e-5, "{kernel:?} {to:?}: {value}");
            }
        }
        let copied = run::<4>(
            &mut resampler,
            &detailed,
            fitted((23, 17), (23, 17), kernel),
        );
        for (copied, original) in copied.iter().zip(&detailed) {
            assert!((copied - original).abs() < 1e-6, "{kernel:?}");
        }
    }
}

#[test]
fn halving_weighs_neighbours_by_the_widened_kernel() {
    let mut resampler = Resampler::new();
    let source = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

    let halved = run::<1>(
        &mut resampler,
        &source,
        fitted((8, 1), (4, 1), Kernel::Bilinear),
    );

    let expected = [0.75 / 1.75, 0.25 / 2.0, 0.0, 0.0];
    for (halved, expected) in halved.iter().zip(expected) {
        assert!((halved - expected).abs() < 1e-6, "{halved} vs {expected}");
    }
}

#[test]
fn coverage_averages_whole_texels_and_fades_at_a_fractional_edge() {
    let mut resampler = Resampler::new();
    let source = [1.0, 3.0, 5.0, 7.0, 2.0, 2.0, 2.0, 2.0];
    let whole = Job {
        x: Axis::fit(4, 2),
        y: Axis::fit(2, 1),
        footprint: Footprint::Coverage,
        threads: Threads::Calling,
    };
    assert_eq!(run::<1>(&mut resampler, &source, whole), [2.0, 4.0]);

    let (left, x) = Axis::place(2.5, 2, 1.0);
    let (top, y) = Axis::place(-1.0, 1, 1.0);
    assert_eq!((left, top, x.target_len, y.target_len), (2, -1, 3, 1));
    let shifted = Job {
        x,
        y,
        footprint: Footprint::Coverage,
        threads: Threads::Calling,
    };
    assert_eq!(
        run::<1>(&mut resampler, &[1.0, 1.0], shifted),
        [0.5, 1.0, 0.5]
    );
}

#[test]
fn every_level_and_thread_count_gives_the_same_bits() {
    let source = texture(1600, 1400, 4);
    let job = fitted((1600, 1400), (701, 333), Kernel::Lanczos3);
    let reference = run::<4>(&mut Resampler::with_level(Level::baseline()), &source, job);

    for level in Level::supported() {
        let mut resampler = Resampler::with_level(level);
        assert_eq!(
            run::<4>(&mut resampler, &source, job),
            reference,
            "{level:?}"
        );
        let banded = Job {
            threads: Threads::Available,
            ..job
        };
        assert_eq!(
            run::<4>(&mut resampler, &source, banded),
            reference,
            "{level:?}"
        );
    }
    let gray = texture(300, 200, 1);
    let job = fitted((300, 200), (123, 77), Kernel::Bicubic);
    let reference = run::<1>(&mut Resampler::with_level(Level::baseline()), &gray, job);
    for level in Level::supported() {
        assert_eq!(
            run::<1>(&mut Resampler::with_level(level), &gray, job),
            reference,
            "{level:?}"
        );
    }
}

#[test]
fn sharp_kernels_keep_the_mean_and_bound_their_ringing() {
    let mut resampler = Resampler::new();
    let mut edge = vec![0.0f32; 64 * 4];
    edge[32 * 4..].fill(1.0);

    let shrunk = run::<4>(
        &mut resampler,
        &edge,
        fitted((64, 1), (16, 1), Kernel::Lanczos3),
    );

    let mean: f32 = shrunk.iter().sum::<f32>() / shrunk.len() as f32;
    assert!((mean - 0.5).abs() < 0.01, "{mean}");
    assert!(shrunk.iter().all(|value| (-0.12..=1.12).contains(value)));
    assert!(shrunk[..4 * 4].iter().all(|value| value.abs() < 0.01));
}
