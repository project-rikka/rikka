import rikka.space.view;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint SLICES = 2u;
static const uint STEPS = 6u;
static const float WIDEST = 256.0;
static const float FIGURE_RADIUS = 0.6;
static const float FIGURE_DEPTH = 0.75;

struct Falloff {
    float scale;
    float offset;
};

float horizon(SpaceView v, uint depth_image, float2 uv, float3 origin, float3 eye, float lowest, Falloff falloff) {
    if (!space_on_screen(uv)) {
        return lowest;
    }
    int2 pixel = space_pixel(uv, v.size);
    float depth = rikka_read_depth(depth_image, pixel);
    if (depth <= 0.0) {
        return lowest;
    }
    float3 delta = space_position(v, space_uv(pixel, v.size), depth) - origin;
    float distance = length(delta);
    float cosine = dot(delta, eye) / max(distance, 1e-5);
    float weight = clamp(distance * falloff.scale + falloff.offset, 0.0, 1.0);
    return lerp(lowest, cosine, weight);
}

struct Ambient {
    float visibility;
    float3 bent;
};

float3 side_integrals(float h, float cos_normal, float sin_normal) {
    float s = sin(h);
    float c = cos(h);
    float sin_double = 2.0 * s * c;
    float cos_double = c * c - s * s;
    float sin_triple = s * (3.0 - 4.0 * s * s);
    float cos_triple = c * (4.0 * c * c - 3.0);
    float visible = (cos_normal + 2.0 * h * sin_normal - (cos_double * cos_normal + sin_double * sin_normal)) * 0.25;
    float across = 6.0 * (s * cos_normal - c * sin_normal) - (sin_triple * cos_normal - cos_triple * sin_normal)
        - 3.0 * (s * cos_normal + c * sin_normal);
    float toward = -(cos_triple * cos_normal + sin_triple * sin_normal) - 3.0 * (c * cos_normal - s * sin_normal);
    return float3(visible, across, toward);
}

Ambient ground_truth(SpaceView v, uint depth_image, float2 uv, float3 origin, float3 normal, float radius, float2 noise) {
    Ambient open = { 1.0, normal };
    float3 eye = space_toward_eye(v, origin);
    float4 clip = mul(v.projection, float4(origin, 1.0));
    float reach = radius * abs(v.projection[0][0]) * 0.5 * v.size.x / max(clip.w, 1e-5);
    if (reach < 1.0) {
        return open;
    }
    reach = min(reach, WIDEST);
    float range = 0.6 * radius;
    Falloff falloff = { -1.0 / range, (radius - range) / range + 1.0 };

    float visibility = 0.0;
    float3 bent = float3(0.0);
    [unroll]
    for (uint slice = 0u; slice < SLICES; ++slice) {
        float phi = (float(slice) + noise.x) * RIKKA_PI / float(SLICES);
        float2 omega = float2(cos(phi), sin(phi));
        float3 direction = float3(omega.x, -omega.y, 0.0);
        float3 across = direction - dot(direction, eye) * eye;
        float3 axis = normalize(cross(across, eye));
        float3 projected = normal - axis * dot(normal, axis);
        float projected_length = length(projected);
        float cos_normal = clamp(dot(projected, eye) / max(projected_length, 1e-4), 0.0, 1.0);
        float turn = sign(dot(across, projected));
        float normal_angle = turn * acos(cos_normal);
        float sin_normal = turn * sqrt(1.0 - cos_normal * cos_normal);
        float low_ahead = cos(normal_angle + RIKKA_PI * 0.5);
        float low_behind = cos(normal_angle - RIKKA_PI * 0.5);
        float ahead = low_ahead;
        float behind = low_behind;
        for (uint step = 0u; step < STEPS; ++step) {
            float t = (float(step) + noise.y) / float(STEPS);
            float pixels = max(t * t * reach, float(step) + 1.0);
            float2 offset = omega * pixels / v.size;
            ahead = max(ahead, horizon(v, depth_image, uv + offset, origin, eye, low_ahead, falloff));
            behind = max(behind, horizon(v, depth_image, uv - offset, origin, eye, low_behind, falloff));
        }
        float h_ahead = acos(clamp(ahead, -1.0, 1.0));
        float h_behind = -acos(clamp(behind, -1.0, 1.0));
        h_ahead = normal_angle + min(h_ahead - normal_angle, RIKKA_PI * 0.5);
        h_behind = normal_angle + max(h_behind - normal_angle, -RIKKA_PI * 0.5);
        float3 integrals = side_integrals(h_ahead, cos_normal, sin_normal)
            + side_integrals(h_behind, cos_normal, sin_normal);
        visibility += projected_length * integrals.x;
        float3 side = across * rsqrt(max(dot(across, across), 1e-12));
        bent += projected_length * (side * (integrals.y + 16.0 * sin_normal) + eye * (integrals.z + 8.0 * cos_normal));
    }
    Ambient result;
    result.visibility = clamp(visibility / float(SLICES), 0.0, 1.0);
    float length_squared = dot(bent, bent);
    result.bent = length_squared > 1e-12 ? bent * rsqrt(length_squared) : normal;
    return result;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = global_id.xy;
    if (any(texel >= push.size)) {
        return;
    }
    RikkaFrameRef frame = push.frame;
    RikkaScene scene = frame.look.scene;
    RikkaSpace space = frame.look.space;
    SpaceView v = space_view(frame);
    int2 pixel = min(int2(texel) * 2, int2(v.size) - 1);
    float depth = rikka_read_depth(scene.depth, pixel);
    if (depth <= 0.0) {
        rikka_storage_rgba16f[push.target][texel] = float4(1.0, 0.0, 0.0, 0.0);
        return;
    }
    float2 uv = space_uv(pixel, v.size);
    float3 origin = space_position(v, uv, depth);
    RikkaSurfaceSample surface = rikka_read_surface(scene.surface, pixel);
    float3 normal = normalize(mul((float3x3)v.view, surface.normal));

    uint cell = (texel.x & 3u) + (texel.y & 3u) * 4u;
    uint shuffled = (cell * 7u + 3u) & 15u;
    float2 noise = float2((float(shuffled) + 0.5) / 16.0, fract(float(cell) * 0.6180339 + 0.25));

    bool figure = surface.figure != 0u;
    float radius = space.occlusion_radius * (figure ? FIGURE_RADIUS : 1.0);
    Ambient ambient = { 1.0, normal };
    if (space.occlusion_strength > 0.0) {
        ambient = ground_truth(v, scene.depth, uv, origin, normal, radius, noise);
        if (figure) {
            ambient.visibility = lerp(1.0, ambient.visibility, FIGURE_DEPTH);
            ambient.bent = normalize(lerp(normal, ambient.bent, FIGURE_DEPTH));
        }
        ambient.visibility = pow(ambient.visibility, space.occlusion_strength);
    }
    float3 bent = mul(ambient.bent, (float3x3)v.view);
    rikka_storage_rgba16f[push.target][texel] =
        float4(ambient.visibility, space_encode_direction(bent), -origin.z);
}
