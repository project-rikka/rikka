import rikka.space.view;

struct Push {
    RikkaFrameRef frame;
    uint color;
    uint depth;
    uint surface;
    uint identity;
    uint flags;
    uint samples;
    float cavity_valley;
    float cavity_ridge;
    float cavity_distance;
    float cavity_attenuation;
    float curvature_ridge;
    float curvature_valley;
    float scale;
    float3 outline;
    uint seed;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint FLAG_CAVITY = 1u;
static const uint FLAG_CURVATURE = 2u;
static const uint FLAG_OUTLINE = 4u;
static const uint MAX_SAMPLES = 512u;

uint id_at(int2 pixel, int2 last) {
    uint id = rikka_identity_images[push.identity].Load(int3(clamp(pixel, int2(0), last), 0)).r;
    return id & RIKKA_IDENTITY_MASK;
}

float3 view_normal(SpaceView v, int2 pixel, int2 last) {
    uint packed = rikka_identity_images[push.surface].Load(int3(clamp(pixel, int2(0), last), 0)).x;
    float3 world = rikka_decode_octahedral(packed);
    return normalize(mul(v.view, float4(world, 0.0)).xyz);
}

float depth_at(float2 uv, float2 size) {
    int2 pixel = clamp(int2(uv * size), int2(0), int2(size) - 1);
    return rikka_images[push.depth].Load(int3(pixel, 0)).r;
}

float noise(int2 pixel, float2 shift) {
    float2 at = float2(pixel) + shift;
    return frac(52.9829189 * frac(dot(at, float2(0.06711056, 0.00583715))));
}

float radical_inverse(uint bits) {
    return float(reversebits(bits)) * 2.3283064365386963e-10;
}

void cavity_compute(SpaceView v, int2 pixel, float2 size, float depth, float3 normal,
                    out float cavities, out float edges) {
    cavities = 0.0;
    edges = 0.0;
    float2 screenco = (float2(pixel) + 0.5) / size;
    float3 position = space_position(v, screenco, depth);
    float homcoord = v.projection[3][2] * position.z + v.projection[3][3];
    float2 offset = float2(v.projection[0][0], v.projection[1][1]) * push.cavity_distance / homcoord * 0.5;
    float shift = float(push.seed % 64u) * 5.588238;
    float phi = noise(pixel, float2(shift)) * RIKKA_TAU;
    float2 rot_x = float2(cos(phi), sin(phi));
    float2 rot_y = float2(-rot_x.y, rot_x.x);
    uint count = min(push.samples, MAX_SAMPLES);
    float jitter = (noise(pixel, float2(shift + 31.0, 17.0)) - 0.5) / float(max(count, 1u));
    for (uint i = 0u; i < count; i++) {
        float r = frac((float(i) + 0.5) / float(count));
        float angle = radical_inverse(i) * RIKKA_TAU;
        float3 sample_coord = float3(cos(angle), sin(angle), r);
        float2 dir_jittered = float2(dot(sample_coord.xy, rot_x), dot(sample_coord.xy, rot_y));
        dir_jittered *= sample_coord.z + jitter;
        float2 uvcoords = screenco + dir_jittered * offset;
        if (any(abs(uvcoords - 0.5) > float2(0.5))) {
            continue;
        }
        float s_depth = depth_at(uvcoords, size);
        bool is_background = s_depth <= 0.0;
        s_depth = is_background ? depth : s_depth;
        float3 s_pos = space_position(v, uvcoords, s_depth);
        if (is_background) {
            s_pos.z -= push.cavity_distance;
        }
        float3 dir = s_pos - position;
        float len = length(dir);
        float f_cavities = dot(dir, normal);
        float f_edge = -f_cavities;
        float f_bias = 0.05 * len + 0.0001;
        float attenuation = 1.0 / (len * (1.0 + len * len * push.cavity_attenuation));
        if (f_cavities > -f_bias) {
            cavities += f_cavities * attenuation;
        }
        if (f_edge > f_bias) {
            edges += f_edge * attenuation;
        }
    }
    float inverse = 1.0 / float(max(count, 1u));
    cavities = clamp(cavities * inverse * push.cavity_valley, 0.0, 1.0);
    edges = edges * inverse * push.cavity_ridge;
}

float curvature_soft_clamp(float curvature, float control) {
    if (curvature < 0.5 / control) {
        return curvature * (1.0 - curvature * control);
    }
    return 0.25 / control;
}

float curvature_compute(SpaceView v, int2 pixel, int2 last) {
    int step = max(int(round(push.scale)), 1);
    int2 up = pixel + int2(0, step);
    int2 down = pixel - int2(0, step);
    int2 right = pixel + int2(step, 0);
    int2 left = pixel - int2(step, 0);
    uint object_up = id_at(up, last);
    uint object_down = id_at(down, last);
    uint object_right = id_at(right, last);
    uint object_left = id_at(left, last);
    if (object_up != object_down || object_right != object_left) {
        return 0.0;
    }
    if (object_up == object_right && object_right == 0u) {
        return 0.0;
    }
    float normal_up = -view_normal(v, up, last).y;
    float normal_down = -view_normal(v, down, last).y;
    float normal_right = view_normal(v, right, last).x;
    float normal_left = view_normal(v, left, last).x;
    float normal_diff = (normal_up - normal_down) + (normal_right - normal_left);
    if (normal_diff < 0.0) {
        return -2.0 * curvature_soft_clamp(-normal_diff, push.curvature_valley);
    }
    return 2.0 * curvature_soft_clamp(normal_diff, push.curvature_ridge);
}

float outline_opacity(int2 pixel, int2 last) {
    int step = max(int(round(push.scale)), 1);
    uint center = id_at(pixel, last);
    uint4 adjacent = uint4(
        id_at(pixel + int2(0, step), last),
        id_at(pixel - int2(0, step), last),
        id_at(pixel + int2(step, 0), last),
        id_at(pixel - int2(step, 0), last));
    return 1.0 - dot(float4(adjacent == uint4(center)), float4(0.25));
}

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    SpaceView v = space_view(push.frame);
    int2 pixel = int2(frag_coord.xy);
    float2 size = v.size;
    int2 last = int2(size) - 1;
    float4 color = rikka_images[push.color].Load(int3(pixel, 0));
    float depth = rikka_images[push.depth].Load(int3(pixel, 0)).r;
    if (depth > 0.0) {
        float cavities = 0.0;
        float edges = 0.0;
        float curvature = 0.0;
        if ((push.flags & FLAG_CAVITY) != 0u) {
            cavity_compute(v, pixel, size, depth, view_normal(v, pixel, last), cavities, edges);
        }
        if ((push.flags & FLAG_CURVATURE) != 0u) {
            curvature = curvature_compute(v, pixel, last);
        }
        color.rgb *= clamp((1.0 - cavities) * (1.0 + edges) * (1.0 + curvature), 0.0, 4.0);
    }
    if ((push.flags & FLAG_OUTLINE) != 0u) {
        float opacity = outline_opacity(pixel, last);
        color = float4(push.outline * opacity, opacity) + color * (1.0 - opacity);
    }
    return color;
}
