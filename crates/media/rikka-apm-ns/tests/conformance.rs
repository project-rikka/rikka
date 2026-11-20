use rikka_apm_audio::{BANDS, Frame};
use rikka_apm_ns::{Level, NoiseSuppressor};

use crate::support::Scene;

const FRAMES: usize = 600;
const BLOCK: usize = 100;
const BLOCKS: usize = FRAMES / BLOCK;
const SAMPLED_FRAMES: [usize; 2] = [299, 599];
const SAMPLED_POSITIONS: [usize; 2] = [0, 80];

struct Golden {
    level: Level,
    channels: usize,
    energy: &'static [f64],
    samples: &'static [f32],
}

const GOLDEN: [Golden; 5] = [
    Golden {
        level: Level::Low,
        channels: 1,
        energy: &[
            840022547.5590812,
            856793707.3692241,
            15677564971.398447,
            51252855632.38104,
            49444067804.40128,
            2800162261.552313,
            203051336.00000402,
            211233624.62660497,
            363608853.6764458,
            764905300.2304826,
            738227112.0857275,
            249068658.24979466,
            52100265.388948165,
            53527422.169326894,
            56633374.23339831,
            62769070.57506723,
            60861189.508259356,
            56568179.870413475,
        ],
        samples: &[
            3517.1711, 3091.433, 6.4725423, -36.49273, -75.3071, 34.014244, 1.2023773, -74.12021,
            -155.23758, 80.441025, -14.721894, -94.847725,
        ],
    },
    Golden {
        level: Level::Moderate,
        channels: 1,
        energy: &[
            291356421.4782771,
            239316354.33121845,
            17202460469.65962,
            57162570318.271194,
            56028378265.442116,
            2963991185.356658,
            52638968.022220366,
            52808406.15665124,
            137533906.57439834,
            353574207.2356876,
            332843074.76968056,
            88149649.7700324,
            13533726.855941596,
            13381855.542331723,
            17686330.149261955,
            24507922.27210256,
            22673781.856362093,
            19257446.48061543,
        ],
        samples: &[
            3797.5042, 3179.0474, 4.559069, -25.704409, -53.04411, 23.958635, 1.1300895,
            -27.449678, -77.61879, 40.220512, -7.360947, -47.423862,
        ],
    },
    Golden {
        level: Level::High,
        channels: 1,
        energy: &[
            133139445.01614662,
            68412072.49735993,
            16795312797.357082,
            56954774517.467804,
            55550315987.89682,
            2739476642.253217,
            21843172.034841307,
            14463010.554607268,
            67148509.90447819,
            206636275.01116213,
            188802566.15702447,
            55778253.15826513,
            5641447.823718969,
            3667488.6085062036,
            7079345.064006577,
            13119958.625999782,
            11180920.629473316,
            12261776.708355777,
        ],
        samples: &[
            3842.2422, 3101.9226, 3.555404, -20.045662, -41.366615, 18.684216, -15.557333,
            -11.512402, -64.66021, 33.505634, -6.1320252, -39.506374,
        ],
    },
    Golden {
        level: Level::VeryHigh,
        channels: 1,
        energy: &[
            68823050.57230428,
            33227144.987055548,
            16474798286.188993,
            56799603190.74441,
            55114247289.40446,
            2619931457.9394765,
            14935049.568791356,
            10229826.342611479,
            51229834.19881171,
            166445108.12642455,
            149593512.4340435,
            43798986.51845404,
            3859841.633864311,
            2593643.4043997545,
            5239175.753898587,
            10404628.268221827,
            8645343.036296804,
            9599501.566152917,
        ],
        samples: &[
            3852.2551, 3039.8472, 3.1669219, -17.855368, -36.84668, 16.64268, -5.435778,
            -13.472018, -56.551212, 29.303713, -5.363012, -34.551903,
        ],
    },
    Golden {
        level: Level::High,
        channels: 2,
        energy: &[
            74899914.46232712,
            55992248.28486106,
            16088618336.42694,
            56232559846.461235,
            54312339591.165596,
            2410817019.6724634,
            18416516.324076366,
            14153613.290767014,
            66448062.19422558,
            204979829.1375073,
            184854541.03482655,
            52917963.531748995,
            4747995.086871362,
            3586189.4582122155,
            6994357.745349343,
            12942333.316720422,
            10893427.785879044,
            11581881.30765443,
            75644015.53861664,
            56151592.993260086,
            14727403170.137981,
            51373340641.67039,
            49180550827.6262,
            2243199576.3495154,
            18678607.840623967,
            14338874.316967737,
            70865382.84056073,
            204320176.14480224,
            186241640.13820717,
            52153304.05956263,
            4701057.955863689,
            3616126.720114694,
            7006272.86945814,
            12830061.621422939,
            10809128.110881515,
            11614639.84088941,
        ],
        samples: &[
            3790.4182, 2721.8564, 3.4808717, -19.625443, -40.49944, 18.292536, -3148.4312,
            -3658.5227, -87.56971, 100.66937, -26.171232, -9.104175, -28.67051, -18.491817,
            -64.66021, 33.505634, -6.1320252, -39.506374, -58.229412, 85.239655, 37.882534,
            9.803856, -1.8619934, -38.27304,
        ],
    },
];

fn close(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance * expected.abs().max(1.0)
}

#[test]
fn matches_webrtc_for_every_level() {
    for golden in &GOLDEN {
        let mut suppressor = NoiseSuppressor::new(golden.level, golden.channels);
        let mut frame = Frame::new(golden.channels);
        let mut scene = Scene::new(golden.channels);
        let mut energy = vec![0.0f64; golden.channels * BANDS * BLOCKS];
        let mut samples = Vec::new();

        for index in 0..FRAMES {
            scene.fill(&mut frame);
            suppressor.analyze(&frame);
            suppressor.process(&mut frame);
            for channel in 0..golden.channels {
                for (band, output) in frame.bands(channel).iter().enumerate() {
                    energy[(channel * BANDS + band) * BLOCKS + index / BLOCK] += output
                        .iter()
                        .map(|&sample| f64::from(sample) * f64::from(sample))
                        .sum::<f64>();
                    if SAMPLED_FRAMES.contains(&index) {
                        samples.extend(SAMPLED_POSITIONS.map(|position| output[position]));
                    }
                }
            }
        }

        let case = format!("{:?} with {} channel(s)", golden.level, golden.channels);
        for (slot, (&actual, &expected)) in energy.iter().zip(golden.energy).enumerate() {
            assert!(
                close(actual, expected, 1e-5),
                "{case}: energy {slot} is {actual}, oracle {expected}"
            );
        }
        assert_eq!(samples.len(), golden.samples.len(), "{case}");
        for (slot, (&actual, &expected)) in samples.iter().zip(golden.samples).enumerate() {
            assert!(
                close(f64::from(actual), f64::from(expected), 1e-4),
                "{case}: sample {slot} is {actual}, oracle {expected}"
            );
        }
    }
}
