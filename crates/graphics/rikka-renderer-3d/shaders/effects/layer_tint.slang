import rikka;

struct Parameters {
    [Default(1.0)] uint mask;
    [Default(0.5)] [Range(0.0, 1.0)] float amount;
    [Default(1.0, 0.45, 0.1)] [Color] float3 color;
    [Default(1.0)] [Range(0.0, 1.0)] [Group("depth")] float nearest;
    [Default(0.0)] [Range(0.0, 1.0)] [Group("depth")] float farthest;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint layers;
    uint depth;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void tint(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    uint layers = rikka_effect_load_layers(push.layers, pixel);
    float depth = rikka_effect_load(push.depth, pixel).r;
    bool chosen = (layers & parameters.mask) != 0u
        && depth > 0.0
        && depth <= parameters.nearest
        && depth >= parameters.farthest;
    if (chosen) {
        float amount = clamp(parameters.amount, 0.0, 1.0);
        color.rgb = lerp(color.rgb, parameters.color * color.a, amount);
    }
    rikka_effect_store(push.target, pixel, color);
}
