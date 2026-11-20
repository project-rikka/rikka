import rikka.effects.log_curves;

struct Parameters {
    [Default(1.0, 1.0, 1.0)] [Range(0.0, 4.0)] float3 slope;
    [Default(0.0, 0.0, 0.0)] [Range(-1.0, 1.0)] float3 offset;
    [Default(1.0, 1.0, 1.0)] [Range(0.1, 4.0)] float3 power;
    [Default(1.0)] [Range(0.0, 4.0)] float saturation;
    [Options("linear|acescct")] [Default(0)] uint space;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint ACESCCT = 1u;

bool neutral(Parameters parameters) {
    return all(parameters.slope == 1.0) && all(parameters.offset == 0.0)
        && all(parameters.power == 1.0) && parameters.saturation == 1.0;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void decide(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    if (neutral(parameters)) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float3 value = rikka_effect_straight(color);
    if (parameters.space == ACESCCT) {
        value = rikka_to_acescct(value);
    }
    value = value * parameters.slope + parameters.offset;
    if (any(parameters.power != 1.0)) {
        value = pow(max(value, float3(0.0)), parameters.power);
    }
    float luma = dot(value, float3(0.2126, 0.7152, 0.0722));
    value = luma + parameters.saturation * (value - luma);
    if (parameters.space == ACESCCT) {
        value = rikka_from_acescct(value);
    }
    rikka_effect_store(push.target, pixel, float4(value * color.a, color.a));
}
