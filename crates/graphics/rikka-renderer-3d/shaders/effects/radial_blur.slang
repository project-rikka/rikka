import rikka.effect;

struct Parameters {
    [Options("zoom|spin")] [Default(0)] uint mode;
    [Default(0.5, 0.5)] [Range(0.0, 1.0)] float2 center;
    [Default(0.12)] [Range(0.0, 1.0)] float amount;
    [Default(0.12)] [Range(0.0, 1.0)] [Group("falloff")] float inner;
    [Default(0.45)] [Range(0.0, 1.0)] [Group("falloff")] float softness;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int TAPS = 12;
static const uint SPIN = 1u;

float4 smear(int2 pixel, float span) {
    Parameters parameters = *push.parameters;
    float2 uv = rikka_effect_uv(pixel, push.size);
    float aspect = rikka_effect_aspect(push.size);
    float2 offset = (uv - parameters.center) * float2(aspect, 1.0);
    float corner = length(float2(aspect, 1.0) * 0.5);
    float reach = smoothstep(parameters.inner, parameters.inner + max(parameters.softness, 1e-3), length(offset) / corner);
    float length_scale = span * parameters.amount * reach;
    float4 sum = float4(0.0);
    for (int tap = 0; tap < TAPS; ++tap) {
        float t = (float(tap) / float(TAPS - 1) - 0.5) * length_scale;
        float2 moved = parameters.mode == SPIN
            ? rikka_effect_rotate(offset, t * RIKKA_PI)
            : offset * (1.0 - t);
        sum += rikka_effect_sample(push.source, parameters.center + moved / float2(aspect, 1.0));
    }
    return sum / float(TAPS);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void fine(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    rikka_effect_store(push.target, int2(id.xy), smear(int2(id.xy), 1.0 / float(TAPS)));
}

[shader("compute")]
[numthreads(8, 8, 1)]
void coarse(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    rikka_effect_store(push.target, int2(id.xy), smear(int2(id.xy), 1.0));
}
