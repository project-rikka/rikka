import rikka.effect;

struct Parameters {
    [Default(0.6)] [Range(0.0, 1.0)] float sharpness;
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float3 tap(int2 pixel) {
    return clamp(rikka_effect_straight(rikka_effect_load_clamped(push.source, pixel, push.size)), 0.0, 1.0);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void sharpen(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    float3 a = tap(pixel + int2(-1, -1));
    float3 b = tap(pixel + int2(0, -1));
    float3 c = tap(pixel + int2(1, -1));
    float3 d = tap(pixel + int2(-1, 0));
    float3 e = clamp(rikka_effect_straight(color), 0.0, 1.0);
    float3 f = tap(pixel + int2(1, 0));
    float3 g = tap(pixel + int2(-1, 1));
    float3 h = tap(pixel + int2(0, 1));
    float3 i = tap(pixel + int2(1, 1));
    float3 cross_min = min(min(min(d, e), min(f, b)), h);
    float3 cross_max = max(max(max(d, e), max(f, b)), h);
    float3 ring_min = min(cross_min, min(min(a, c), min(g, i)));
    float3 ring_max = max(cross_max, max(max(a, c), max(g, i)));
    float3 low = cross_min + ring_min;
    float3 high = cross_max + ring_max;
    float3 amplitude = sqrt(clamp(min(low, 2.0 - high) / max(high, float3(1e-5)), 0.0, 1.0));
    float peak = -1.0 / lerp(8.0, 5.0, clamp(parameters.sharpness, 0.0, 1.0));
    float3 weight = amplitude * peak;
    float3 sharpened = clamp((b * weight + d * weight + f * weight + h * weight + e) / (1.0 + 4.0 * weight), 0.0, 1.0);
    float3 result = lerp(rikka_effect_straight(color), sharpened, clamp(parameters.amount, 0.0, 1.0));
    rikka_effect_store(push.target, pixel, float4(result * color.a, color.a));
}
