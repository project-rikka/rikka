import rikka.effect;

struct Parameters {
    [Default(1.33)] [Range(0.5, 2.0)] float squeeze;
    [Default(0.15)] [Range(-1.0, 1.0)] [Label("horizontal bow")] float bow;
    [Default(0.5, 0.5)] [Range(0.0, 1.0)] float2 center;
    [Default(1.0)] [Range(0.0, 1.0)] float fit;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float bend(Parameters parameters, float2 offset) {
    return (1.0 - 0.5 * parameters.bow * dot(offset, offset)) / max(parameters.squeeze, 0.05);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void squeeze(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float aspect = rikka_effect_aspect(push.size);
    float2 half_extent = float2(aspect, 1.0) * 0.5;
    float diagonal = length(half_extent);
    float2 corner = half_extent / diagonal;
    float2 side = float2(half_extent.x / diagonal, 0.0);
    float widest = max(bend(parameters, corner), bend(parameters, side));
    float zoom = lerp(1.0, max(widest, 1.0), clamp(parameters.fit, 0.0, 1.0));
    float2 uv = rikka_effect_uv(pixel, push.size);
    float2 offset = (uv - parameters.center) * float2(aspect, 1.0) / diagonal;
    float2 bent = float2(offset.x * bend(parameters, offset), offset.y) / zoom;
    float2 source = parameters.center + bent * diagonal / float2(aspect, 1.0);
    bool inside = all(source >= 0.0) && all(source <= 1.0);
    float4 color = inside ? rikka_effect_sample(push.source, source) : float4(0.0);
    rikka_effect_store(push.target, pixel, color);
}
