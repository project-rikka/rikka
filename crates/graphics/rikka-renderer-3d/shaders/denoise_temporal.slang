import rikka.denoise;

struct Push {
    RikkaFrameRef frame;
    uint noisy;
    uint history;
    uint moments_history;
    uint guide_history;
    uint guide;
    uint target;
    uint moments_target;
    uint2 size;
    uint scale;
    uint channels;
    float longest;
    uint fresh;
    uint clamped;
    uint tiles;
    uint confidence;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float DEPTH_TOLERANCE = 0.1;
static const float NORMAL_LIMIT = 0.8;
static const float CLAMP_SPREAD = 1.5;
static const float SETTLED = 4.0;

float4 load(uint image, int2 texel) {
    return rikka_fetch(rikka_images[NonUniformResourceIndex(image)], texel, 0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = global_id.xy;
    if (any(texel >= push.size)) {
        return;
    }
    RikkaFrameRef frame = push.frame;
    RikkaViewRef view = frame.view;
    RikkaScene scene = frame.look.scene;
    uint2 full = uint2(view.viewport);
    int2 pixel = rikka_full_pixel(texel, push.scale, full);
    float4 noisy = load(push.noisy, int2(texel));
    RikkaGuide here = rikka_guide_load(push.guide, int2(texel));
    if (!here.covered) {
        rikka_storage_rgba16f[push.target][texel] = noisy;
        rikka_storage_rgba16f[push.moments_target][texel] = float4(0.0);
        return;
    }
    if (rikka_tile_settled(push.tiles, texel, push.size)) {
        float settled = rikka_signal_luminance(noisy, push.channels);
        rikka_storage_rgba16f[push.target][texel] = noisy;
        rikka_storage_rgba16f[push.moments_target][texel] =
            float4(settled, settled * settled, SETTLED, 0.0);
        return;
    }

    int2 highest = int2(push.size) - 1;
    float sum = 0.0;
    float square = 0.0;
    float4 low = noisy;
    float4 high = noisy;
    float4 mean = float4(0.0);
    float4 mean_square = float4(0.0);
    [unroll]
    for (int y = -1; y <= 1; ++y) {
        [unroll]
        for (int x = -1; x <= 1; ++x) {
            float4 tap = load(push.noisy, clamp(int2(texel) + int2(x, y), int2(0), highest));
            float l = rikka_signal_luminance(tap, push.channels);
            sum += l;
            square += l * l;
            mean += tap;
            mean_square += tap * tap;
        }
    }
    sum /= 9.0;
    square /= 9.0;
    mean /= 9.0;
    float4 deviation = sqrt(max(mean_square / 9.0 - mean * mean, float4(0.0)));
    low = mean - CLAMP_SPREAD * deviation;
    high = mean + CLAMP_SPREAD * deviation;

    float4 history = float4(0.0);
    float4 moments = float4(0.0);
    float weight = 0.0;
    if (push.fresh == 0u && view.cut == 0u) {
        float2 velocity = rikka_read_surface(scene.surface, pixel).velocity;
        float2 uv = (float2(pixel) + 0.5) * view.inverse_viewport;
        float2 previous = rikka_history_uv(view, uv, velocity);
        float3 world = rikka_world_of(frame, pixel);
        float4 previous_clip = mul(scene.previous_view_projection, float4(world, 1.0));
        float previous_depth = previous_clip.w;
        float2 before = rikka_previous_size(view, push.scale);
        int2 last = int2(before) - 1;
        float2 at = previous * before - 0.5;
        int2 base = int2(floor(at));
        float2 f = at - float2(base);
        [unroll]
        for (int y = 0; y < 2; ++y) {
            [unroll]
            for (int x = 0; x < 2; ++x) {
                int2 tap = base + int2(x, y);
                if (any(tap < int2(0)) || any(tap > last)) {
                    continue;
                }
                float4 guide = load(push.guide_history, tap);
                if (guide.w <= 0.0) {
                    continue;
                }
                bool same = abs(guide.w - previous_depth) < DEPTH_TOLERANCE * previous_depth
                    && dot(guide.xyz, here.normal) > NORMAL_LIMIT;
                if (!same) {
                    continue;
                }
                float w = (x == 0 ? 1.0 - f.x : f.x) * (y == 0 ? 1.0 - f.y : f.y);
                history += load(push.history, tap) * w;
                moments += load(push.moments_history, tap) * w;
                weight += w;
            }
        }
    }

    float length = 0.0;
    if (weight > 1e-3) {
        history /= weight;
        moments /= weight;
        length = moments.z;
    }
    if (push.confidence != 0u) {
        length = min(length, saturate(noisy.a) * push.longest);
    }
    if (push.clamped != 0u && length > 0.0) {
        history = clamp(history, low, high);
    }
    length = min(length + 1.0, max(push.longest, 1.0));
    float alpha = 1.0 / length;
    float l = rikka_signal_luminance(noisy, push.channels);
    float2 accumulated = lerp(moments.xy, float2(l, l * l), alpha);
    float4 color = lerp(history, noisy, alpha);
    float variance = length < SETTLED
        ? max(square - sum * sum, 0.0) * (SETTLED / length)
        : max(accumulated.y - accumulated.x * accumulated.x, 0.0);
    rikka_storage_rgba16f[push.target][texel] = color;
    rikka_storage_rgba16f[push.moments_target][texel] = float4(accumulated, length, variance);
}
