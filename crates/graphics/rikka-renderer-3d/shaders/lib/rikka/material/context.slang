__exported import rikka.common;
__exported import rikka.push;
__exported import rikka.fragment;
__exported import rikka.parameters;
__exported import rikka.material.texture;

[vk::constant_id(1)] const uint RIKKA_FEATURES = 0xffffffffu;

bool rikka_feature(uint bit) {
    return (RIKKA_FEATURES & (1u << bit)) != 0u;
}

struct RikkaContext {
    RikkaFrameRef frame;
    RikkaViewRef view;
    RikkaInstance instance;
    RikkaMaterial material;
    RikkaFigure figure;
    uint pass;
    bool looked;
    float time;
};

RikkaContext rikka_context(uint instance_index) {
    RikkaFrameRef frame = rikka_push.frame;
    RikkaContext c;
    c.frame = frame;
    c.view = frame.view;
    c.instance = frame.instances[instance_index];
    c.material = frame.materials[c.instance.material];
    c.looked = frame.look.scene.enabled != 0u;
    uint figure = c.instance.figure < frame.look.scene.figure_count ? c.instance.figure : 0u;
    c.figure = frame.figures[figure];
    c.pass = rikka_push.flags;
    c.time = c.view.time;
    return c;
}

T* rikka_block<T>(RikkaContext c) {
    return (T*)(c.frame.blocks + uint64_t(c.material.block) * uint64_t(RIKKA_BLOCK_ALIGNMENT));
}

static const float RIKKA_FACIAL_RELIEF = 0.25;

bool rikka_facial(RikkaContext c) {
    return (c.instance.flags & RIKKA_FACIAL) != 0u;
}

float3 rikka_shadow_receiver(RikkaContext c, float3 world_position) {
    if (!rikka_facial(c)) {
        return world_position;
    }
    return world_position + c.figure.head_forward * (c.figure.head_radius * RIKKA_FACIAL_RELIEF);
}

struct RikkaPixel {
    float3 world_position;
    float3 geometric_normal;
    float4 tangent;
    float2 uv;
    float4 color;
    float4 wet;
    float3 view;
    float view_depth;
    float4 frag_coord;
    bool front_facing;
    bool hull;
    uint traced;

    float3 normal_map(float3 tangent_normal) {
        float3 n = geometric_normal;
        float3 t = normalize(tangent.xyz - n * dot(n, tangent.xyz));
        float3 b = cross(n, t) * tangent.w;
        return normalize(mul(rikka_columns(t, b, n), tangent_normal));
    }
};

RikkaPixel rikka_pixel(RikkaContext c, RikkaVaryings v, float4 frag_coord, bool front_facing, bool hull) {
    rikka_clip(c.frame, v.world_position);
    RikkaPixel p;
    p.world_position = v.world_position;
    p.geometric_normal = normalize(v.normal);
    if ((c.instance.flags & RIKKA_DOUBLE_SIDED) != 0u && !front_facing) {
        p.geometric_normal = -p.geometric_normal;
    }
    if (hull) {
        p.geometric_normal = -p.geometric_normal;
    }
    p.tangent = float4(normalize(v.tangent.xyz), v.tangent.w);
    p.uv = v.uv;
    p.color = v.color;
    p.wet = v.wet;
    p.view = normalize(c.view.camera_position - v.world_position);
    p.view_depth = dot(v.world_position - c.view.camera_position, c.view.camera_forward);
    p.frag_coord = frag_coord;
    p.front_facing = front_facing;
    p.hull = hull;
    p.traced = RIKKA_UNTRACED;
    return p;
}

float3 rikka_unpack_normal(float4 texel) {
    float2 xy = texel.xy * 2.0 - 1.0;
    return float3(xy, sqrt(max(1.0 - dot(xy, xy), 0.0)));
}

float rikka_coverage(RikkaContext c, float alpha) {
    if ((c.pass & RIKKA_PASS_MASKED) != 0u && alpha < c.material.alpha_cutoff) {
        discard;
    }
    return (c.instance.flags & RIKKA_TRANSPARENT) != 0u || (c.pass & RIKKA_PASS_SEE_THROUGH) != 0u ? alpha : 1.0;
}

struct RikkaCoverage {
    float2 uv;
};

struct RikkaHullVertex {
    float3 world_position;
    float3 normal;
    float3 smoothed_normal;
    float4 tangent;
    float2 uv;
    float4 color;
};

struct RikkaHull {
    float3 direction;
    float width;
    float depth_offset;
};

RikkaHull rikka_no_hull(RikkaHullVertex vertex) {
    RikkaHull h;
    h.direction = vertex.normal;
    h.width = 0.0;
    h.depth_offset = 0.0;
    return h;
}
