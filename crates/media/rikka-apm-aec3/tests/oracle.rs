use crate::support::banded::{Scenario, Signals, Variant, run};

struct Golden {
    name: &'static str,
    scenario: Scenario,
    energies: &'static [f64],
    samples: &'static [(usize, usize, usize, usize, f32)],
    metrics: (f32, f32, i32),
}

const ENERGY_TOLERANCE: f64 = 1e-4;
const SAMPLE_TOLERANCE: f32 = 1e-2;
const METRIC_TOLERANCE: f64 = 1e-3;

fn goldens() -> Vec<Golden> {
    vec![
        Golden {
            name: "mono with a delay jump",
            scenario: Scenario {
                delay_change_frame: Some(900),
                near_frames: 350..450,
                ..Scenario::mono(1500)
            },
            energies: &[
                34800840.03728689,
                68184829.78020775,
                92526378.21526729,
                6152615246.807481,
                6308949253.119179,
                166274143.60594118,
                163902054.89465192,
                167601008.54550472,
                168550569.5401887,
                1129231019.925043,
                137996505.9087633,
                140086012.55513293,
                140986901.01726115,
                141837130.99140698,
                149790059.71587357,
            ],
            samples: &[
                (757, 0, 0, 5, 42.2456),
                (757, 0, 0, 77, -126.08295),
                (757, 0, 0, 140, 26.758688),
                (757, 0, 1, 33, 19.882057),
                (757, 0, 2, 99, 0.8901767),
                (1497, 0, 0, 5, -60.084675),
                (1497, 0, 0, 77, -191.73337),
                (1497, 0, 0, 140, -7.0089474),
                (1497, 0, 1, 33, -57.67611),
                (1497, 0, 2, 99, 4.777951),
            ],
            metrics: (19.297379, 32.06286, 56),
        },
        Golden {
            name: "stereo render and capture",
            scenario: Scenario {
                render_channels: 2,
                capture_channels: 2,
                near_frames: 700..800,
                ..Scenario::mono(1500)
            },
            energies: &[
                705654184.2382443,
                335546377.02774173,
                80389194.9580982,
                152766243.3713319,
                206737515.74936873,
                247164008.80622202,
                381167280.5493224,
                16903448179.072079,
                830190877.2343937,
                395484735.30523646,
                391249777.99955195,
                391790380.34207976,
                395561871.57454246,
                414614527.20872575,
                416370892.5128068,
            ],
            samples: &[
                (757, 0, 0, 5, 124.06325),
                (757, 0, 0, 77, -274.74435),
                (757, 0, 0, 140, -809.4946),
                (757, 0, 1, 33, -78.43982),
                (757, 0, 2, 99, 0.6699163),
                (757, 1, 0, 5, -293.0739),
                (757, 1, 0, 77, -371.22974),
                (757, 1, 0, 140, 705.0981),
                (757, 1, 1, 33, -60.10387),
                (757, 1, 2, 99, -0.2807317),
                (1497, 0, 0, 5, 3.369031),
                (1497, 0, 0, 77, -47.687103),
                (1497, 0, 0, 140, 30.156408),
                (1497, 0, 1, 33, 29.103575),
                (1497, 0, 2, 99, 13.183156),
                (1497, 1, 0, 5, 41.26229),
                (1497, 1, 0, 77, 40.435917),
                (1497, 1, 0, 140, 116.88963),
                (1497, 1, 1, 33, -12.237446),
                (1497, 1, 2, 99, 10.78674),
            ],
            metrics: (20.0, 31.012024, 12),
        },
        Golden {
            name: "jittery calls, saturation and a gain change",
            scenario: Scenario {
                delay_change_frame: Some(600),
                jitter: true,
                saturation: 400..420,
                echo_path_change_frame: Some(800),
                near_frames: 300..360,
                ..Scenario::mono(1200)
            },
            energies: &[
                34359699.180678755,
                68164872.1886077,
                92601684.19442768,
                9058196651.726122,
                167753742.14811903,
                163704111.37416387,
                2090902778.5797074,
                139494849.0820984,
                140175113.49188405,
                142811815.3227838,
                146138799.5094361,
                147113414.06142592,
            ],
            samples: &[
                (607, 0, 0, 5, 68.342354),
                (607, 0, 0, 77, -12.914343),
                (607, 0, 0, 140, -334.68033),
                (607, 0, 1, 33, 55.906837),
                (607, 0, 2, 99, 2.3397534),
                (1197, 0, 0, 5, 57.780373),
                (1197, 0, 0, 77, 79.26665),
                (1197, 0, 0, 140, -4.5814724),
                (1197, 0, 1, 33, -18.238195),
                (1197, 0, 2, 99, -1.239984),
            ],
            metrics: (17.690317, 32.498154, 44),
        },
        Golden {
            name: "signal dependent erle, adaptive decay and stationarity",
            scenario: Scenario {
                capture_channels: 2,
                variant: Variant::SignalDependentErle,
                delay_change_frame: Some(900),
                near_frames: 350..450,
                ..Scenario::mono(1500)
            },
            energies: &[
                20813372165.80033,
                2496772843.48438,
                2196629719.722571,
                21529006673.80583,
                23461477455.453136,
                1712366620.3294754,
                1857133313.5372367,
                1253523066.6353877,
                689492434.2542131,
                7205527890.362282,
                10674340487.723667,
                895452271.273371,
                587491775.3837214,
                755033405.5558939,
                836967298.9174663,
            ],
            samples: &[
                (757, 0, 0, 5, 37.603188),
                (757, 0, 0, 77, -60.680717),
                (757, 0, 0, 140, 13.711686),
                (757, 0, 1, 33, 19.146614),
                (757, 0, 2, 99, 1.1874515),
                (757, 1, 0, 5, -54.964962),
                (757, 1, 0, 77, 12.074057),
                (757, 1, 0, 140, 23.278946),
                (757, 1, 1, 33, -22.974308),
                (757, 1, 2, 99, -0.4976074),
                (1497, 0, 0, 5, -11.375929),
                (1497, 0, 0, 77, 7.952556),
                (1497, 0, 0, 140, 15.884063),
                (1497, 0, 1, 33, 16.19591),
                (1497, 0, 2, 99, 26.495476),
                (1497, 1, 0, 5, -30.694523),
                (1497, 1, 0, 77, -22.741108),
                (1497, 1, 0, 140, 13.3088455),
                (1497, 1, 1, 33, 25.930387),
                (1497, 1, 2, 99, 22.986937),
            ],
            metrics: (18.976652, 32.07612, 56),
        },
        Golden {
            name: "subband nearend, 8x decimation, high-passed reference and fixed capture delay",
            scenario: Scenario {
                render_channels: 2,
                variant: Variant::Tuned,
                saturation: 500..510,
                near_frames: 350..450,
                ..Scenario::mono(1500)
            },
            energies: &[
                223794156.76228565,
                898013040.798642,
                63715268.91584199,
                1037682375.4344903,
                1602939246.984446,
                136028346.1338178,
                198129757.55248812,
                201320315.38959333,
                204462557.18724403,
                201424862.076174,
                194532041.52935186,
                196150205.4649607,
                203809245.64053923,
                207067250.47969756,
                214615548.10512766,
            ],
            samples: &[
                (757, 0, 0, 5, 51.27599),
                (757, 0, 0, 77, 107.658066),
                (757, 0, 0, 140, 46.28979),
                (757, 0, 1, 33, -5.4789143),
                (757, 0, 2, 99, -33.63052),
                (1497, 0, 0, 5, 17.422436),
                (1497, 0, 0, 77, 35.70292),
                (1497, 0, 0, 140, 89.74053),
                (1497, 0, 1, 33, -24.64515),
                (1497, 0, 2, 99, -7.9020934),
            ],
            metrics: (15.968641, 7.4800506, 28),
        },
        Golden {
            name: "external delay estimator",
            scenario: Scenario {
                variant: Variant::ExternalDelay,
                buffer_delay_ms: Some(24),
                near_frames: 350..450,
                ..Scenario::mono(1200)
            },
            energies: &[
                33510284.37008688,
                68080125.86113222,
                91734849.90779401,
                6411587757.910921,
                6142420182.150443,
                166582320.8015558,
                164414440.56517503,
                167456841.21608928,
                168180768.85808176,
                167180393.794187,
                166971204.2233043,
                164428580.93023866,
            ],
            samples: &[
                (607, 0, 0, 5, -106.14599),
                (607, 0, 0, 77, 34.838676),
                (607, 0, 0, 140, 119.12704),
                (607, 0, 1, 33, 93.043434),
                (607, 0, 2, 99, 11.10577),
                (1197, 0, 0, 5, -13.95276),
                (1197, 0, 0, 77, -29.19614),
                (1197, 0, 0, 140, -59.711296),
                (1197, 0, 1, 33, 14.741909),
                (1197, 0, 2, 99, -14.55537),
            ],
            metrics: (19.834053, 32.84547, 24),
        },
        Golden {
            name: "reported audio buffer delay",
            scenario: Scenario {
                buffer_delay_ms: Some(35),
                near_frames: 350..450,
                ..Scenario::mono(1200)
            },
            energies: &[
                35078429.46038759,
                68141188.61387584,
                92565770.43930568,
                6163803381.99866,
                6189208098.813299,
                166291234.42141008,
                163884978.3946505,
                167624042.66346103,
                168507537.45831195,
                166976099.2154428,
                167234714.09139642,
                164385076.24213254,
            ],
            samples: &[
                (607, 0, 0, 5, -99.4471),
                (607, 0, 0, 77, 35.606808),
                (607, 0, 0, 140, 119.34587),
                (607, 0, 1, 33, 93.0898),
                (607, 0, 2, 99, 11.313899),
                (1197, 0, 0, 5, 10.924988),
                (1197, 0, 0, 77, -24.908524),
                (1197, 0, 0, 140, -61.479164),
                (1197, 0, 1, 33, 14.741909),
                (1197, 0, 2, 99, -16.387047),
            ],
            metrics: (19.808752, 32.69113, 28),
        },
    ]
}

fn check(golden: &Golden) -> Vec<String> {
    let signals = Signals::new(&golden.scenario);
    let result = run(&golden.scenario, &signals);
    let mut failures = Vec::new();
    for (chunk, (frames, expected)) in result.output.chunks(100).zip(golden.energies).enumerate() {
        let energy: f64 = frames
            .iter()
            .flatten()
            .flatten()
            .flatten()
            .map(|v| f64::from(*v) * f64::from(*v))
            .sum();
        if (energy - expected).abs() > ENERGY_TOLERANCE * expected {
            failures.push(format!(
                "{}: frames {}.. carry energy {energy}, WebRTC {expected}",
                golden.name,
                chunk * 100
            ));
        }
    }
    for &(frame, channel, band, k, expected) in golden.samples {
        let got = result.output[frame][channel][band][k];
        if (got - expected).abs() > SAMPLE_TOLERANCE {
            failures.push(format!(
                "{}: frame {frame} ch {channel} band {band} [{k}] is {got}, WebRTC {expected}",
                golden.name
            ));
        }
    }
    let metrics = result.metrics.last().copied().unwrap();
    let (erl, erle, delay_ms) = golden.metrics;
    if (metrics.echo_return_loss - f64::from(erl)).abs() > METRIC_TOLERANCE
        || (metrics.echo_return_loss_enhancement - f64::from(erle)).abs() > METRIC_TOLERANCE
        || metrics.delay_ms != delay_ms
    {
        failures.push(format!(
            "{}: metrics {metrics:?}, WebRTC ({erl}, {erle}, {delay_ms})",
            golden.name
        ));
    }
    failures
}

#[test]
fn capture_output_matches_webrtc() {
    let goldens = goldens();
    let failures: Vec<String> = std::thread::scope(|scope| {
        let runs: Vec<_> = goldens
            .iter()
            .map(|golden| scope.spawn(|| check(golden)))
            .collect();
        runs.into_iter()
            .flat_map(|run| run.join().unwrap())
            .collect()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
