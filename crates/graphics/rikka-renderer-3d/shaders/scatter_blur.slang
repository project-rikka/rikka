import rikka.space.view;

struct Push {
    RikkaFrameRef frame;
    uint source;
    uint profile;
    uint base;
    uint target;
    uint2 size;
    float2 direction;
    float reach;
    uint _pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint TAPS = 7u;
static const float OFFSETS[TAPS] = { 0.0, 0.3, 0.8, 1.5, 2.5, 4.0, 6.0 };
static const float WIDTHS[TAPS] = { 0.3, 0.4, 0.6, 0.85, 1.25, 1.75, 2.0 };
static const float DEPTH_FALLOFF = 4.0;

float3 burley(float r, float3 d) {
    return (exp(-r / d) + exp(-r / (3.0 * d))) / (8.0 * d);
}

float4 base_at(int2 pixel) {
    return push.base == RIKKA_NONE ? float4(0.0) : rikka_fetch(rikka_images[push.base], pixel, 0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 pixel = int2(global_id.xy);
    if (any(uint2(pixel) >= push.size)) {
        return;
    }
    float4 centre = rikka_fetch(rikka_images[push.source], pixel, 0);
    float4 base = base_at(pixel);
    if (centre.a <= 0.0) {
        rikka_storage_rgba16f[push.target][uint2(pixel)] = push.base == RIKKA_NONE ? centre : base;
        return;
    }
    RikkaFrameRef frame = push.frame;
    RikkaViewRef view = frame.view;
    float3 d = max(rikka_fetch(rikka_images[push.profile], pixel, 0).rgb, float3(1e-6));
    float reach = max(max(d.x, d.y), d.z);
    uint depth_image = frame.look.scene.depth;
    float depth = rikka_read_depth(depth_image, pixel);
    float distance = depth > 0.0 ? rikka_linear_depth(view, depth) : 1.0;
    bool orthographic = view.projection[3][3] == 1.0;
    float pixels = view.viewport.y * abs(view.projection[1][1]) * 0.5 / (orthographic ? 1.0 : max(distance, 1e-3));
    float scale = reach * pixels;
    float3 result = centre.rgb;
    if (scale * OFFSETS[TAPS - 1u] >= 1.0) {
        float3 sum = float3(0.0);
        float3 total = float3(0.0);
        int2 highest = int2(push.size) - 1;
        float stagger = rikka_temporal_noise(view, float2(pixel) + push.direction * 17.0) - 0.5;
        [unroll]
        for (uint tap = 0u; tap < TAPS; ++tap) {
            float r = tap == 0u ? 0.0 : OFFSETS[tap] + stagger * 0.5 * WIDTHS[tap];
            float3 weight = burley(r * reach, d) * WIDTHS[tap];
            float span = min(r * scale, push.reach);
            [unroll]
            for (int side = 0; side < (tap == 0u ? 1 : 2); ++side) {
                float facing = side == 0 ? 1.0 : -1.0;
                int2 at = clamp(int2(round(float2(pixel) + push.direction * span * facing)), int2(0), highest);
                float4 sample = rikka_fetch(rikka_images[push.source], at, 0);
                if (sample.a <= 0.0) {
                    continue;
                }
                float sample_depth = rikka_read_depth(depth_image, at);
                float gap = sample_depth > 0.0 ? abs(rikka_linear_depth(view, sample_depth) - distance) : 1e6;
                float along = clamp(1.0 - gap / (DEPTH_FALLOFF * reach), 0.0, 1.0);
                sum += sample.rgb * weight * along;
                total += weight * along;
            }
        }
        result = sum / max(total, float3(1e-8));
    }
    float4 written = push.base == RIKKA_NONE ? float4(result, centre.a) : float4(base.rgb + result, base.a);
    rikka_storage_rgba16f[push.target][uint2(pixel)] = written;
}
