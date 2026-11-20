use rustc_hash::FxHashMap;

const NONE: usize = usize::MAX;

struct MergeEdge {
    low: u32,
    left: usize,
    right: usize,
    dissolvable: bool,
    length: f64,
}

struct MergeFace {
    verts: Vec<u32>,
    edges: Vec<usize>,
    merged: bool,
}

pub(crate) struct MergeRules<D, L> {
    pub(crate) dissolvable: D,
    pub(crate) length_squared: L,
    pub(crate) allow_repeated_verts: bool,
}

pub(crate) fn merge_triangles<D, L>(
    triangles: &[[u32; 3]],
    rules: MergeRules<D, L>,
) -> Vec<Vec<u32>>
where
    D: Fn(u32, u32) -> bool,
    L: Fn(u32, u32) -> f64,
{
    let mut faces: Vec<MergeFace> = Vec::with_capacity(triangles.len());
    let mut edges: Vec<MergeEdge> = Vec::with_capacity(triangles.len() * 2);
    let mut lookup: FxHashMap<(u32, u32), usize> = FxHashMap::default();
    for triangle in triangles {
        let face = faces.len();
        let mut face_edges = Vec::with_capacity(3);
        for i in 0..3 {
            let (a, b) = (triangle[i], triangle[(i + 1) % 3]);
            let key = (a.min(b), a.max(b));
            let index = *lookup.entry(key).or_insert_with(|| {
                edges.push(MergeEdge {
                    low: key.0,
                    left: NONE,
                    right: NONE,
                    dissolvable: (rules.dissolvable)(key.0, key.1),
                    length: (rules.length_squared)(key.0, key.1),
                });
                edges.len() - 1
            });
            let edge = &mut edges[index];
            let slot = match edge.low == a {
                true => &mut edge.left,
                false => &mut edge.right,
            };
            match *slot == NONE {
                true => *slot = face,
                false => edge.dissolvable = false,
            }
            face_edges.push(index);
        }
        faces.push(MergeFace {
            verts: triangle.to_vec(),
            edges: face_edges,
            merged: false,
        });
    }
    let mut order: Vec<usize> = (0..edges.len()).filter(|&e| edges[e].dissolvable).collect();
    order.sort_by(|&a, &b| edges[b].length.total_cmp(&edges[a].length).then(a.cmp(&b)));
    for edge in order {
        let (left, right) = (edges[edge].left, edges[edge].right);
        if left == NONE || right == NONE || left == right {
            continue;
        }
        if !can_dissolve(&faces, &edges, edge, rules.allow_repeated_verts) {
            continue;
        }
        splice(&mut faces, &mut edges, edge);
    }
    faces
        .into_iter()
        .filter(|face| !face.merged)
        .map(|face| face.verts)
        .collect()
}

fn other_face(edge: &MergeEdge, face: usize) -> usize {
    match edge.left == face {
        true => edge.right,
        false => edge.left,
    }
}

fn can_dissolve(
    faces: &[MergeFace],
    edges: &[MergeEdge],
    edge: usize,
    allow_repeated: bool,
) -> bool {
    let (left, right) = (edges[edge].left, edges[edge].right);
    let a = &faces[left];
    let b = &faces[right];
    if a.edges
        .iter()
        .any(|&other| other != edge && other_face(&edges[other], left) == right)
    {
        return false;
    }
    if allow_repeated {
        return true;
    }
    let position = a.edges.iter().position(|&e| e == edge).unwrap_or(0);
    let ends = [a.verts[position], a.verts[(position + 1) % a.verts.len()]];
    !a.verts
        .iter()
        .any(|v| !ends.contains(v) && b.verts.contains(v))
}

fn splice(faces: &mut [MergeFace], edges: &mut [MergeEdge], edge: usize) {
    let (left, right) = (edges[edge].left, edges[edge].right);
    let a_start = faces[left]
        .edges
        .iter()
        .position(|&e| e == edge)
        .unwrap_or(0);
    let b_start = faces[right]
        .edges
        .iter()
        .position(|&e| e == edge)
        .unwrap_or(0);
    let a_len = faces[left].verts.len();
    let b_len = faces[right].verts.len();
    let mut verts = Vec::with_capacity(a_len + b_len - 2);
    let mut face_edges = Vec::with_capacity(a_len + b_len - 2);
    verts.extend_from_slice(&faces[left].verts[..a_start]);
    face_edges.extend_from_slice(&faces[left].edges[..a_start]);
    let mut index = (b_start + 1) % b_len;
    while index != b_start {
        let vert = faces[right].verts[index];
        let moved = faces[right].edges[index];
        verts.push(vert);
        face_edges.push(moved);
        let record = &mut edges[moved];
        match record.left == right {
            true => record.left = left,
            false => record.right = left,
        }
        index = (index + 1) % b_len;
    }
    verts.extend_from_slice(&faces[left].verts[a_start + 1..]);
    face_edges.extend_from_slice(&faces[left].edges[a_start + 1..]);
    faces[right].merged = true;
    faces[left].verts = verts;
    faces[left].edges = face_edges;
    edges[edge].left = NONE;
    edges[edge].right = NONE;
}
