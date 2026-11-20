import rikka.effect;

struct Parameters {
    [Default(1.25)] [Range(0.25, 4.0)] [Label("band (stops)")] float stops;
    [Default(0.15)] [Range(0.0, 1.0)] float softness;
    [Default(0.5)] [Range(0.0, 1.0)] [Label("band offset")] float offset;
    [Default(1)] uint mask;
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint layers;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void band(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    bool chosen = (rikka_effect_load_layers(push.layers, pixel) & parameters.mask) != 0u;
    float3 straight = max(rikka_effect_straight(color), float3(0.0));
    float luma = rikka_luma(straight);
    if (!chosen || luma <= 1e-6) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float stops = max(parameters.stops, 0.05);
    float level = log2(luma) / stops + parameters.offset;
    float step_index = floor(level);
    float soft = max(parameters.softness, 1e-3);
    float stepped = step_index + smoothstep(1.0 - soft, 1.0, level - step_index) + 0.5 - parameters.offset;
    float3 shaded = straight * exp2(stepped * stops - log2(luma));
    float3 result = lerp(straight, shaded, clamp(parameters.amount, 0.0, 1.0));
    rikka_effect_store(push.target, pixel, float4(result * color.a, color.a));
}
