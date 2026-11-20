import rikka.effect;

struct Parameters {
    [Default(0.0)] [Range(-3.14159, 3.14159)] [Hint("angle")] float angle;
    [Default(40.0)] [Range(0.0, 600.0)] [Label("length (px at 1080p)")] float length;
    [Options("centred|trailing")] [Default(0)] uint anchor;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int TAPS = 12;
static const uint TRAILING = 1u;

float4 smear(int2 pixel, float fraction, bool first) {
    Parameters parameters = *push.parameters;
    float2 size = float2(push.size);
    float span = max(parameters.length, 0.0) * rikka_effect_scale(push.size);
    float2 direction = float2(cos(parameters.angle), -sin(parameters.angle));
    float2 center = float2(pixel) + 0.5;
    if (first && parameters.anchor == TRAILING) {
        center -= direction * span * 0.5;
    }
    float2 step = direction * span * fraction;
    float4 sum = float4(0.0);
    for (int tap = 0; tap < TAPS; ++tap) {
        float t = (float(tap) + 0.5) / float(TAPS) - 0.5;
        sum += rikka_effect_sample(push.source, (center + step * t) / size);
    }
    return sum / float(TAPS);
}

bool still(int2 pixel) {
    if (push.parameters.length * rikka_effect_scale(push.size) > 1e-3) {
        return false;
    }
    rikka_effect_store(push.target, pixel, rikka_effect_load(push.source, pixel));
    return true;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void fine(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size) || still(pixel)) {
        return;
    }
    rikka_effect_store(push.target, pixel, smear(pixel, 1.0 / float(TAPS), true));
}

[shader("compute")]
[numthreads(8, 8, 1)]
void coarse(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size) || still(pixel)) {
        return;
    }
    rikka_effect_store(push.target, pixel, smear(pixel, 1.0, false));
}
