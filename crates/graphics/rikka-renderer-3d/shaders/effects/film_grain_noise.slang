import rikka.effects.film_grain;

struct Push {
    Parameters* parameters;
    uint target;
    uint2 size;
    float time;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void noise(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size) || push.parameters.amount <= 0.0) {
        return;
    }
    Parameters parameters = *push.parameters;
    uint seed = uint(floor(max(push.time, 0.0) * max(parameters.rate, 0.0)));
    float cell = max(parameters.size * rikka_effect_scale(push.size), 0.5);
    float2 position = (float2(pixel) + 0.5) / cell;
    float3 grain = float3(rikka_grain_noise(position, seed, 0u));
    if (parameters.color > 0.0) {
        float3 colored = float3(
            rikka_grain_noise(position, seed, 1u),
            rikka_grain_noise(position, seed, 2u),
            rikka_grain_noise(position, seed, 3u));
        grain = lerp(grain, colored, clamp(parameters.color, 0.0, 1.0));
    }
    rikka_effect_store(push.target, pixel, float4(grain, 1.0));
}
