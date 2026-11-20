import rikka.editor;

struct Push {
    uint ids;
    uint outline_depth;
    uint scene_depth;
    uint flags;
    float occluded_alpha;
    uint pad;
    uint2 size;
    uint4 palette[4];
};

[[vk::push_constant]] ConstantBuffer<Push> push;

struct Output {
    float4 color : SV_Target0;
    float4 segment : SV_Target1;
};

static const uint FLAG_THICK = 1u;
static const uint FLAG_SMOOTH = 2u;
static const uint FLAG_WIRES = 4u;

static const int XPOS = 1;
static const int XNEG = 2;
static const int YPOS = 4;
static const int YNEG = 8;
static const int ALL = XPOS | XNEG | YPOS | YNEG;
static const int NONE = 0;
static const int DIAG_XNEG_YPOS = XNEG | YPOS;
static const int DIAG_XPOS_YPOS = XPOS | YPOS;
static const int DIAG_XPOS_YNEG = XPOS | YNEG;
static const int DIAG_XNEG_YNEG = XNEG | YNEG;
static const int APEX_XPOS = ALL & (~XPOS);
static const int APEX_XNEG = ALL & (~XNEG);
static const int APEX_YPOS = ALL & (~YPOS);
static const int APEX_YNEG = ALL & (~YNEG);

static const float PROXIMITY_OFS = -0.35;
static const float OCCLUSION_EPSILON = 1e-5;
static const float NEVER_OPAQUE = 254.0 / 255.0;

uint id_at(int2 texel) {
    int2 last = int2(push.size) - 1;
    return rikka_identity_images[push.ids].Load(int3(clamp(texel, int2(0), last), 0)).r;
}

float depth_at(uint image, int2 texel) {
    int2 last = int2(push.size) - 1;
    return rikka_images[image].Load(int3(clamp(texel, int2(0), last), 0)).r;
}

bool has_edge(uint id, int2 texel, uint ref, inout uint ref_col, inout int2 depth_texel) {
    if (ref_col == 0u) {
        ref_col = id;
        depth_texel = texel;
    }
    return id != ref;
}

bool4 gather_edges(int2 pixel, float2 offset, uint ref) {
    int2 corner = pixel + int2(offset + 0.5);
    uint4 ids = uint4(
        id_at(corner + int2(-1, 0)),
        id_at(corner),
        id_at(corner + int2(0, -1)),
        id_at(corner + int2(-1, -1)));
    return ids != uint4(ref);
}

float2 rotate_90(float2 v) {
    return float2(v.y, -v.x);
}

float2 rotate_180(float2 v) {
    return float2(-v.x, -v.y);
}

float2 rotate_270(float2 v) {
    return float2(-v.y, v.x);
}

bool4 rotate_90(bool4 v) {
    return v.yzwx;
}

bool4 rotate_180(bool4 v) {
    return v.zwxy;
}

bool4 rotate_270(bool4 v) {
    return v.wxyz;
}

bool line_offset(bool2 edges, float2 ofs, inout float2 line_point) {
    if (all(edges.xy)) {
        line_point.y -= ofs.y;
    } else if (!edges.x) {
        line_point.y += ofs.y;
    } else {
        line_point.x += ofs.x;
        return true;
    }
    return false;
}

void straight_line_dir(bool4 edges1, bool4 edges2, out float2 line_start, out float2 line_end) {
    line_end = float2(1.5, 0.5 + PROXIMITY_OFS);
    line_start = float2(-line_end.x, line_end.y);
    float2 line_ofs = float2(1.0, 0.5);
    if (line_offset(edges1.xw, line_ofs, line_end)) {
        line_offset(edges1.yz, line_ofs, line_end);
    }
    line_ofs = float2(-line_ofs.x, line_ofs.y);
    if (line_offset(edges2.yz, line_ofs, line_start)) {
        line_offset(edges2.xw, line_ofs, line_start);
    }
}

float2 diag_offset(bool4 edges) {
    if (all(edges.wz)) {
        return float2(2.5, 0.5);
    } else if (!edges.x && !edges.w) {
        return float2(0.5, 2.5);
    } else if (edges.w) {
        return float2(2.5, 0.5);
    }
    return float2(0.5, 2.5);
}

void diag_dir(bool4 edges1, bool4 edges2, out float2 line_start, out float2 line_end) {
    edges2 = !edges2;
    edges2 = rotate_180(edges2);
    line_end = diag_offset(edges1);
    line_end += diag_offset(edges2);
    if (line_end.x == line_end.y) {
        line_start = float2(-1.0, 1.0) * PROXIMITY_OFS * 0.4;
    } else if (line_end.x > line_end.y) {
        line_start = float2(0.0, PROXIMITY_OFS);
    } else {
        line_start = -float2(PROXIMITY_OFS, 0.0);
    }
    line_end += line_start;
}

float4 line_data(float2 line_start, float2 line_end) {
    float2 edge = line_start - line_end;
    float len = length(edge);
    if (len > 0.0) {
        edge /= len;
        float2 perp = float2(-edge.y, edge.x);
        return float4(perp, dot(perp, -line_start), 1.0);
    }
    return float4(1.0, -1.0, 0.0, 1.0);
}

float4 palette_color(uint index) {
    return unpackUnorm4x8ToFloat(push.palette[index >> 2u][index & 3u]);
}

[shader("fragment")]
Output fragment_main(float4 frag_coord: SV_Position) {
    int2 pixel = int2(frag_coord.xy);
    uint ref = id_at(pixel);
    uint ref_col = ref;
    int2 depth_texel = pixel;

    bool thick = (push.flags & FLAG_THICK) != 0u;
    bool has_edge_pos_x = has_edge(id_at(pixel + int2(1, 0)), pixel + int2(1, 0), ref, ref_col, depth_texel);
    bool has_edge_neg_x = has_edge(id_at(pixel - int2(1, 0)), pixel - int2(1, 0), ref, ref_col, depth_texel);
    bool has_edge_pos_y = has_edge(id_at(pixel + int2(0, 1)), pixel + int2(0, 1), ref, ref_col, depth_texel);
    bool has_edge_neg_y = has_edge(id_at(pixel - int2(0, 1)), pixel - int2(0, 1), ref, ref_col, depth_texel);
    if (thick && !(has_edge_pos_x || has_edge_neg_x || has_edge_pos_y || has_edge_neg_y)) {
        has_edge_pos_x = has_edge(id_at(pixel + int2(2, 0)), pixel + int2(2, 0), ref, ref_col, depth_texel);
        has_edge_neg_x = has_edge(id_at(pixel - int2(2, 0)), pixel - int2(2, 0), ref, ref_col, depth_texel);
        has_edge_pos_y = has_edge(id_at(pixel + int2(0, 2)), pixel + int2(0, 2), ref, ref_col, depth_texel);
        has_edge_neg_y = has_edge(id_at(pixel - int2(0, 2)), pixel - int2(0, 2), ref, ref_col, depth_texel);
    }

    if ((push.flags & FLAG_WIRES) != 0u) {
        has_edge_neg_x = false;
        has_edge_pos_y = false;
    }

    int edge_case = 0;
    edge_case += has_edge_pos_x ? XPOS : 0;
    edge_case += has_edge_neg_x ? XNEG : 0;
    edge_case += has_edge_pos_y ? YPOS : 0;
    edge_case += has_edge_neg_y ? YNEG : 0;
    if (ref_col == 0u || edge_case == ALL || edge_case == NONE) {
        discard;
    }

    float ref_depth = depth_at(push.outline_depth, depth_texel);
    float scene_depth = depth_at(push.scene_depth, depth_texel);
    bool occluded = ref_depth < scene_depth * (1.0 - OCCLUSION_EPSILON);
    float4 straight = palette_color(ref_col >> RIKKA_OUTLINE_COLOR_SHIFT);
    float factor = (occluded ? push.occluded_alpha : 1.0) * NEVER_OPAQUE * straight.a;

    Output out;
    out.color = float4(straight.rgb, 1.0) * factor;
    out.segment = float4(0.0);
    if ((push.flags & FLAG_SMOOTH) == 0u) {
        return out;
    }

    float2 line_start = float2(0.0);
    float2 line_end = float2(0.0);
    bool4 extra_edges;
    bool4 extra_edges2;
    switch (edge_case) {
    case YPOS:
        extra_edges = gather_edges(pixel, float2(2.5, 0.5), ref);
        extra_edges2 = gather_edges(pixel, float2(-2.5, 0.5), ref);
        straight_line_dir(extra_edges, extra_edges2, line_start, line_end);
        break;
    case YNEG:
        extra_edges = rotate_180(gather_edges(pixel, float2(-2.5, -0.5), ref));
        extra_edges2 = rotate_180(gather_edges(pixel, float2(2.5, -0.5), ref));
        straight_line_dir(extra_edges, extra_edges2, line_start, line_end);
        line_start = rotate_180(line_start);
        line_end = rotate_180(line_end);
        break;
    case XPOS:
        extra_edges = rotate_90(gather_edges(pixel, float2(0.5, 2.5), ref));
        extra_edges2 = rotate_90(gather_edges(pixel, float2(0.5, -2.5), ref));
        straight_line_dir(extra_edges, extra_edges2, line_start, line_end);
        line_start = rotate_90(line_start);
        line_end = rotate_90(line_end);
        break;
    case XNEG:
        extra_edges = rotate_270(gather_edges(pixel, float2(-0.5, 2.5), ref));
        extra_edges2 = rotate_270(gather_edges(pixel, float2(-0.5, -2.5), ref));
        straight_line_dir(extra_edges, extra_edges2, line_start, line_end);
        line_start = rotate_270(line_start);
        line_end = rotate_270(line_end);
        break;
    case DIAG_XNEG_YPOS:
        extra_edges = gather_edges(pixel, float2(1.5, 1.5), ref);
        extra_edges2 = gather_edges(pixel, float2(-1.5, -1.5), ref);
        diag_dir(extra_edges, extra_edges2, line_start, line_end);
        break;
    case DIAG_XPOS_YNEG:
        extra_edges = rotate_180(gather_edges(pixel, float2(-1.5, -1.5), ref));
        extra_edges2 = rotate_180(gather_edges(pixel, float2(1.5, 1.5), ref));
        diag_dir(extra_edges, extra_edges2, line_start, line_end);
        line_start = rotate_180(line_start);
        line_end = rotate_180(line_end);
        break;
    case DIAG_XPOS_YPOS:
        extra_edges = rotate_90(gather_edges(pixel, float2(1.5, -1.5), ref));
        extra_edges2 = rotate_90(gather_edges(pixel, float2(-1.5, 1.5), ref));
        diag_dir(extra_edges, extra_edges2, line_start, line_end);
        line_start = rotate_90(line_start);
        line_end = rotate_90(line_end);
        break;
    case DIAG_XNEG_YNEG:
        extra_edges = rotate_270(gather_edges(pixel, float2(-1.5, 1.5), ref));
        extra_edges2 = rotate_270(gather_edges(pixel, float2(1.5, -1.5), ref));
        diag_dir(extra_edges, extra_edges2, line_start, line_end);
        line_start = rotate_270(line_start);
        line_end = rotate_270(line_end);
        break;
    case APEX_XPOS:
    case APEX_XNEG:
        line_start = float2(-0.5, 0.0);
        line_end = float2(0.5, 0.0);
        break;
    case APEX_YPOS:
    case APEX_YNEG:
        line_start = float2(0.0, -0.5);
        line_end = float2(0.0, 0.5);
        break;
    default:
        break;
    }
    out.segment = line_data(line_start, line_end);
    return out;
}
