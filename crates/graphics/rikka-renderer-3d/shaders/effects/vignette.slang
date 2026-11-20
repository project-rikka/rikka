import rikka;

struct Parameters {
    [Default(0.5, 0.5)] [Range(0.0, 1.0)] [Group("shape")] float2 center;
    [Default(0.5)] [Range(0.0, 1.0)] float strength;
    [Default(0.35)] [Range(0.0, 1.5)] [Group("shape")] float radius;
    [Default(0.55)] [Range(0.0, 1.5)] [Group("shape")] float softness;
    [Default(1.0)] [Range(0.0, 1.0)] [Group("shape")] float roundness;
    [Default(0.0)] [Range(0.0, 1.0)] [Group("shape")] [Label("squareness")] float squareness;
    [Color] float3 color;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void darken(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    float2 size = float2(push.size);
    float2 uv = rikka_effect_uv(pixel, push.size);
    float aspect = size.x / size.y;
    float2 offset = (uv - parameters.center)
        * float2(lerp(1.0, aspect, clamp(parameters.roundness, 0.0, 1.0)), 1.0);
    float exponent = 2.0 + 6.0 * clamp(parameters.squareness, 0.0, 1.0);
    float2 magnitude = abs(offset);
    float extent = parameters.squareness > 0.0
        ? pow(pow(magnitude.x, exponent) + pow(magnitude.y, exponent), 1.0 / exponent)
        : length(offset);
    float reach = smoothstep(
        parameters.radius,
        parameters.radius + max(parameters.softness, 1e-4),
        extent);
    float kept = 1.0 - clamp(parameters.strength, 0.0, 1.0) * reach;
    color.rgb = color.rgb * kept + parameters.color * (1.0 - kept) * color.a;
    rikka_effect_store(push.target, pixel, color);
}
