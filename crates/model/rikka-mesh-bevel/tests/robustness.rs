use rikka_mesh_bevel::{
    Affect, BevelInput, BevelOptions, CurveProfile, FaceStrength, InnerMiter, Intersection, Limit,
    OuterMiter, ProfilePreset, ProfileType, WeightLayers, WidthType, bevel, bevel_selected,
};
use rikka_mesh_edit::EditMesh;
use rikka_polymesh::{BEVEL_WEIGHT_EDGE, BEVEL_WEIGHT_VERT};
use rikka_test_meshes::{
    EditOperator, Rng, bounds, corpus, outcome, sample_fixtures, selected_edges, selected_verts,
    sweep_chains, sweep_edit,
};

const CASES: usize = 32;
const SAMPLE_CASES: usize = 12;
const CHAINS: usize = 16;
const CHAIN_LENGTH: usize = 4;
const OUTPUT_FACES: f32 = 20_000.0;

fn options(mesh: &EditMesh, rng: &mut Rng) -> BevelOptions {
    let (low, high) = bounds(mesh);
    let size = (high - low).length().max(0.01);
    let width_type = rng.pick(&[
        WidthType::Offset,
        WidthType::Width,
        WidthType::Depth,
        WidthType::Percent,
        WidthType::Absolute,
    ]);
    let width = match (width_type, rng.below(3)) {
        (WidthType::Percent, _) => rng.range(0.0, 100.0),
        (_, 0) => rng.range(0.0, 100.0),
        _ => rng.range(0.0, 0.4) * size,
    };
    let budget = (OUTPUT_FACES / mesh.face_count().max(1) as f32).sqrt() as u32;
    let segments = rng.int(1, budget.clamp(2, 13) - 1);
    let profile_type = match rng.chance(0.25) {
        true => ProfileType::Custom(
            CurveProfile::from_preset(
                rng.pick(&[
                    ProfilePreset::Line,
                    ProfilePreset::SupportLoops,
                    ProfilePreset::Cornice,
                    ProfilePreset::CrownMolding,
                    ProfilePreset::Steps,
                ]),
                segments as usize,
            )
            .with_sample_straight_edges(rng.flip())
            .with_sample_even_lengths(rng.flip()),
        ),
        false => ProfileType::Superellipse,
    };
    BevelOptions {
        affect: rng.pick(&[Affect::Vertices, Affect::Edges]),
        width_type,
        width,
        segments,
        profile_shape: rng.unit(),
        profile_type,
        material: rng.flip().then(|| rng.int(0, 3)),
        harden_normals: rng.flip(),
        clamp_overlap: rng.flip(),
        loop_slide: rng.flip(),
        mark_seam: rng.flip(),
        mark_sharp: rng.flip(),
        miter_outer: rng.pick(&[OuterMiter::Sharp, OuterMiter::Patch, OuterMiter::Arc]),
        miter_inner: rng.pick(&[InnerMiter::Sharp, InnerMiter::Arc]),
        spread: rng.range(0.0, 100.0),
        intersection: rng.pick(&[Intersection::GridFill, Intersection::Cutoff]),
        face_strength: rng.pick(&[
            FaceStrength::None,
            FaceStrength::New,
            FaceStrength::Affected,
            FaceStrength::All,
        ]),
    }
}

fn layers() -> WeightLayers {
    WeightLayers {
        vert: BEVEL_WEIGHT_VERT.to_owned(),
        edge: BEVEL_WEIGHT_EDGE.to_owned(),
    }
}

fn operators() -> Vec<(&'static str, EditOperator)> {
    vec![
        ("bevel_selected", |mesh, rng| {
            let options = options(mesh, rng);
            outcome(bevel_selected(mesh, &options))
        }),
        ("bevel_selection_input", |mesh, rng| {
            let options = options(mesh, rng);
            let mut input = BevelInput::from_selection(mesh);
            if rng.flip() {
                input.weights = Some(layers());
            }
            if rng.flip() && !mesh.vertex_groups().is_empty() {
                input.vertex_group = Some(rng.below(mesh.vertex_groups().len()) as u32);
            }
            outcome(bevel(mesh, &input, &options))
        }),
        ("bevel_edges", |mesh, rng| {
            let options = BevelOptions {
                affect: Affect::Edges,
                ..options(mesh, rng)
            };
            let input = BevelInput::from_edges(mesh, selected_edges(mesh).iter());
            outcome(bevel(mesh, &input, &options))
        }),
        ("bevel_verts", |mesh, rng| {
            let options = BevelOptions {
                affect: Affect::Vertices,
                ..options(mesh, rng)
            };
            let input = BevelInput::from_verts(selected_verts(mesh).iter());
            outcome(bevel(mesh, &input, &options))
        }),
        ("bevel_limit", |mesh, rng| {
            let mut options = options(mesh, rng);
            let limit = match rng.below(4) {
                0 => Limit::None,
                1 => Limit::Angle(rng.range(0.0, std::f32::consts::PI)),
                2 => Limit::Weight,
                _ => Limit::VertexGroup {
                    group: rng.below(mesh.vertex_groups().len() + 1) as u32,
                    invert: rng.flip(),
                },
            };
            options.clamp_overlap = true;
            let input = BevelInput::from_limit(mesh, options.affect, limit, &layers())
                .map_err(|error| error.to_string())?;
            outcome(bevel(mesh, &input, &options))
        }),
    ]
}

#[test]
fn bevel_survives_corpus() {
    sweep_edit("bevel corpus", &corpus(), CASES, &operators());
}

#[test]
fn bevel_survives_samples() {
    sweep_edit(
        "bevel samples",
        &sample_fixtures(),
        SAMPLE_CASES,
        &operators(),
    );
}

#[test]
fn bevel_chains_survive() {
    let mut fixtures = corpus();
    fixtures.extend(sample_fixtures());
    sweep_chains(
        "bevel chains",
        &fixtures,
        CHAINS,
        CHAIN_LENGTH,
        &operators(),
    );
}

#[test]
fn huge_width_beside_short_edge_stays_finite() {
    let points = [
        [-12.006182, -43.443996, -0.8760767],
        [-12.006127, -43.44401, -0.8760096],
        [-12.005862, -43.44409, -0.8756605],
        [12.036528, 46.325752, 0.94627815],
        [12.036521, 46.325706, 0.9462504],
        [7.044257, 30.520979, 1.0899296],
        [7.348343, 30.820797, 0.9591608],
        [12.036495, 46.325596, 0.94625646],
        [7.1124954, 30.965792, 1.1301138],
    ];
    let faces: [[usize; 4]; 4] = [[3, 6, 8, 7], [7, 8, 5, 4], [0, 3, 7, 1], [1, 7, 4, 2]];
    let mut mesh = EditMesh::new();
    let verts: Vec<_> = points
        .iter()
        .map(|&point| mesh.add_vert(glam::Vec3::from_array(point)))
        .collect();
    for face in faces {
        let corners: Vec<_> = face.iter().map(|&index| verts[index]).collect();
        mesh.add_face(&corners).expect("fixture face");
    }
    mesh.update_normals();
    mesh.set_select_mode(rikka_mesh_edit::SelectMode::EDGE);
    mesh.select_all(rikka_mesh_edit::SelectAction::Select);
    let options = BevelOptions {
        width_type: WidthType::Width,
        width: 46.897804,
        profile_shape: 0.8618358,
        miter_outer: OuterMiter::Arc,
        intersection: Intersection::Cutoff,
        ..BevelOptions::default()
    };
    bevel_selected(&mut mesh, &options).expect("bevel");
    rikka_test_meshes::check(&mesh, true).expect("finite valid mesh");
}
