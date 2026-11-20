import rikka.post;
import rikka.post.focus;

struct Push {
    uint color;
    uint coc;
    uint tiles;
    uint far;
    uint near;
    uint coverage;
    uint seed;
    uint2 size;
    float blades;
    float rotation;
    float highlights;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int MAX_RINGS = 5;

float covered(float radius, float distance) {
    return clamp(radius - distance + 0.5, 0.0, 1.0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 texel = int2(global_id.xy);
    int2 size = int2(push.size);
    if (any(texel >= size)) {
        return;
    }
    float4 center = post_fetch(push.color, texel);
    float own = post_fetch(push.coc, texel).r;
    float reach = post_fetch(push.tiles, texel / POST_FOCUS_TILE).y;
    if (reach < 0.5) {
        post_store(push.far, texel, center);
        post_store(push.near, texel, center);
        post_store_scalar(push.coverage, texel, 0.0);
        return;
    }

    int rings = clamp(int(ceil(reach / 3.0)), 1, MAX_RINGS);
    float count = float(1 + 4 * rings * (rings + 1));
    float density = reach * reach / count;
    float spin = post_random(uint3(uint2(texel), push.seed)) * RIKKA_TAU;
    float own_far = max(own, 0.0);

    float4 far_sum = float4(0.0);
    float far_weight = 0.0;
    float4 near_sum = float4(0.0);
    float near_weight = 0.0;
    float near_cover = 0.0;

    for (int ring = 0; ring <= rings; ++ring) {
        int taps = max(ring * 8, 1);
        float radius = reach * float(ring) / float(rings);
        for (int tap = 0; tap < taps; ++tap) {
            float angle = spin + (float(tap) + 0.5 * float(ring & 1)) * RIKKA_TAU / float(taps);
            float2 offset = radius * float2(cos(angle), sin(angle));
            float2 at = float2(texel) + 0.5 + offset;
            int2 nearest = clamp(int2(floor(at)), int2(0), size - 1);
            float4 sampled = post_fetch(push.color, nearest);
            float coc = post_fetch(push.coc, nearest).r;
            float distance = radius / post_polygon(angle + RIKKA_PI, push.blades, push.rotation);
            float glow = 1.0 + push.highlights
                * smoothstep(0.8, 4.0, rikka_luma(sampled.rgb) / max(sampled.a, 1e-3));

            float far_radius = min(max(coc, 0.0), own_far);
            float far_cover = covered(far_radius, distance) * density / max(far_radius * far_radius, 0.25);
            far_sum += sampled * far_cover * glow;
            far_weight += far_cover * glow;

            float near_radius = max(-coc, 0.0);
            float near = covered(near_radius, distance) * smoothstep(0.5, 1.5, near_radius)
                * density / max(near_radius * near_radius, 0.25);
            near_sum += sampled * near * glow;
            near_weight += near * glow;
            near_cover += near;
        }
    }
    post_store(push.far, texel, far_weight > 1e-4 ? far_sum / far_weight : center);
    post_store(push.near, texel, near_weight > 1e-4 ? near_sum / near_weight : center);
    post_store_scalar(push.coverage, texel, clamp(near_cover, 0.0, 1.0));
}
