use std::collections::HashMap;
use std::str::FromStr;
use std::sync::OnceLock;

use glam::{Vec2, Vec3};

const PATH: &str = "crates/model/rikka-mesh-bevel/tests/oracle/blender.txt";
const MICRO: f32 = 1.0e-6;

#[derive(Default)]
pub(crate) struct Input {
    pub(crate) verts: Vec<Vec3>,
    pub(crate) edges: Vec<[u32; 2]>,
    pub(crate) faces: Vec<Vec<u32>>,
    pub(crate) uvs: Vec<Vec2>,
    pub(crate) seams: Vec<u32>,
    pub(crate) sharp: Vec<u32>,
    pub(crate) smooth: Vec<u32>,
    pub(crate) edge_weights: Vec<(u32, f32)>,
    pub(crate) vert_weights: Vec<(u32, f32)>,
    pub(crate) group: Vec<(u32, f32)>,
}

#[derive(Default)]
pub(crate) struct Output {
    pub(crate) counts: [u32; 3],
    pub(crate) selected: Option<[u32; 3]>,
    pub(crate) faces: Vec<Vec<Vec3>>,
    pub(crate) uvs: Vec<Vec<Vec2>>,
    pub(crate) normals: Vec<Vec<Vec3>>,
    pub(crate) materials: Vec<i32>,
    pub(crate) seams: Vec<[Vec3; 2]>,
    pub(crate) sharp: Vec<[Vec3; 2]>,
    pub(crate) strength: Vec<i32>,
}

pub(crate) struct Case {
    pub(crate) name: String,
    pub(crate) input: String,
    pub(crate) verts: Vec<u32>,
    pub(crate) edges: Vec<u32>,
    pub(crate) output: Output,
}

pub(crate) struct Summary {
    pub(crate) name: String,
    pub(crate) input: String,
    pub(crate) verts: Vec<u32>,
    pub(crate) edges: Vec<u32>,
    pub(crate) counts: [u32; 3],
    pub(crate) position_sum: Vec3,
    pub(crate) edge_length: f32,
    pub(crate) face_sizes: Vec<(u32, u32)>,
}

#[derive(Default)]
struct Oracle {
    inputs: HashMap<String, Input>,
    cases: HashMap<String, Case>,
    summaries: HashMap<String, Summary>,
}

enum Record {
    Input(String, Input),
    Case(Case),
    Summary(Summary),
}

fn numbers<T: FromStr>(values: &[&str]) -> Vec<T>
where
    T::Err: std::fmt::Debug,
{
    values
        .iter()
        .map(|value| value.parse().expect("oracle number"))
        .collect()
}

fn triple<T: FromStr + Copy>(values: &[&str]) -> [T; 3]
where
    T::Err: std::fmt::Debug,
{
    let parsed: Vec<T> = numbers(values);
    [parsed[0], parsed[1], parsed[2]]
}

fn micro_points(values: &[&str]) -> Vec<Vec3> {
    numbers::<i32>(values)
        .chunks(3)
        .map(|chunk| Vec3::new(chunk[0] as f32, chunk[1] as f32, chunk[2] as f32) * MICRO)
        .collect()
}

fn micro_uvs(values: &[&str]) -> Vec<Vec2> {
    numbers::<i32>(values)
        .chunks(2)
        .map(|chunk| Vec2::new(chunk[0] as f32, chunk[1] as f32) * MICRO)
        .collect()
}

fn weighted(values: &[&str]) -> Vec<(u32, f32)> {
    values
        .chunks(2)
        .map(|pair| {
            (
                pair[0].parse().expect("oracle index"),
                pair[1].parse().expect("oracle weight"),
            )
        })
        .collect()
}

fn input_field(input: &mut Input, key: &str, values: &[&str]) {
    match key {
        "verts" => {
            input.verts = numbers::<f32>(values)
                .chunks(3)
                .map(Vec3::from_slice)
                .collect();
        }
        "edges" => {
            input.edges = numbers::<u32>(values)
                .chunks(2)
                .map(|pair| [pair[0], pair[1]])
                .collect();
        }
        "faces" => {
            let indices: Vec<u32> = numbers(values);
            let mut at = 0;
            while at < indices.len() {
                let count = indices[at] as usize;
                input.faces.push(indices[at + 1..=at + count].to_vec());
                at += count + 1;
            }
        }
        "uvs" => {
            input.uvs = numbers::<f32>(values)
                .chunks(2)
                .map(Vec2::from_slice)
                .collect();
        }
        "seams" => input.seams = numbers(values),
        "sharp" => input.sharp = numbers(values),
        "smooth" => input.smooth = numbers(values),
        "edge_weights" => input.edge_weights = weighted(values),
        "vert_weights" => input.vert_weights = weighted(values),
        "group" => input.group = weighted(values),
        other => panic!("unknown oracle input field {other}"),
    }
}

fn edge_pairs(values: &[&str]) -> Vec<[Vec3; 2]> {
    micro_points(values)
        .chunks(2)
        .map(|pair| [pair[0], pair[1]])
        .collect()
}

fn case_field(case: &mut Case, key: &str, values: &[&str]) {
    let output = &mut case.output;
    match key {
        "select_verts" => case.verts = numbers(values),
        "select_edges" => case.edges = numbers(values),
        "counts" => output.counts = triple(values),
        "selected" => output.selected = Some(triple(values)),
        "face" => output.faces.push(micro_points(values)),
        "uv" => output.uvs.push(micro_uvs(values)),
        "normal" => output.normals.push(micro_points(values)),
        "materials" => output.materials = numbers(values),
        "seams" => output.seams = edge_pairs(values),
        "sharp" => output.sharp = edge_pairs(values),
        "strength" => output.strength = numbers(values),
        other => panic!("unknown oracle case field {other}"),
    }
}

fn summary_field(summary: &mut Summary, key: &str, values: &[&str]) {
    match key {
        "select_verts" => summary.verts = numbers(values),
        "select_edges" => summary.edges = numbers(values),
        "counts" => summary.counts = triple(values),
        "position_sum" => summary.position_sum = Vec3::from_array(triple(values)),
        "edge_length" => summary.edge_length = values[0].parse().expect("oracle length"),
        "face_sizes" => {
            summary.face_sizes = numbers::<u32>(values)
                .chunks(2)
                .map(|pair| (pair[0], pair[1]))
                .collect();
        }
        other => panic!("unknown oracle summary field {other}"),
    }
}

impl Oracle {
    fn store(&mut self, record: Record) {
        match record {
            Record::Input(name, input) => {
                self.inputs.insert(name, input);
            }
            Record::Case(case) => {
                self.cases.insert(case.name.clone(), case);
            }
            Record::Summary(summary) => {
                self.summaries.insert(summary.name.clone(), summary);
            }
        }
    }

    fn parse(text: &str) -> Oracle {
        let mut oracle = Oracle::default();
        let mut current: Option<Record> = None;
        for line in text.lines() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let Some((&key, values)) = tokens.split_first() else {
                continue;
            };
            let header = match key {
                "input" => Some(Record::Input(values[0].to_owned(), Input::default())),
                "case" => Some(Record::Case(Case {
                    name: values[0].to_owned(),
                    input: values[1].to_owned(),
                    verts: Vec::new(),
                    edges: Vec::new(),
                    output: Output::default(),
                })),
                "summary" => Some(Record::Summary(Summary {
                    name: values[0].to_owned(),
                    input: values[1].to_owned(),
                    verts: Vec::new(),
                    edges: Vec::new(),
                    counts: [0; 3],
                    position_sum: Vec3::ZERO,
                    edge_length: 0.0,
                    face_sizes: Vec::new(),
                })),
                _ => None,
            };
            if let Some(header) = header {
                if let Some(record) = current.replace(header) {
                    oracle.store(record);
                }
                continue;
            }
            match current.as_mut() {
                Some(Record::Input(_, input)) => input_field(input, key, values),
                Some(Record::Case(case)) => case_field(case, key, values),
                Some(Record::Summary(summary)) => summary_field(summary, key, values),
                None => panic!("oracle field {key} outside a record"),
            }
        }
        if let Some(record) = current {
            oracle.store(record);
        }
        oracle
    }
}

fn oracle() -> &'static Oracle {
    static ORACLE: OnceLock<Oracle> = OnceLock::new();
    ORACLE.get_or_init(|| {
        let text = std::fs::read_to_string(PATH)
            .unwrap_or_else(|error| panic!("failed to read {PATH}: {error}"));
        Oracle::parse(&text)
    })
}

pub(crate) fn input(name: &str) -> &'static Input {
    oracle()
        .inputs
        .get(name)
        .unwrap_or_else(|| panic!("unknown oracle input {name}"))
}

pub(crate) fn case(name: &str) -> &'static Case {
    oracle()
        .cases
        .get(name)
        .unwrap_or_else(|| panic!("unknown oracle case {name}"))
}

pub(crate) fn summary(name: &str) -> &'static Summary {
    oracle()
        .summaries
        .get(name)
        .unwrap_or_else(|| panic!("unknown oracle summary {name}"))
}
