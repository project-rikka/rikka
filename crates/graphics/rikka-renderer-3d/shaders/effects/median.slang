import rikka.effect;

struct Parameters {
    [Default(0.0)] [Range(0.0, 4.0)] [Label("threshold (relative contrast, 0 = everywhere)")] float threshold;
    [Default(1.0)] [Range(0.0, 1.0)] float amount;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

void order(inout float4 a, inout float4 b) {
    float4 low = min(a, b);
    b = max(a, b);
    a = low;
}

void least3(inout float4 a, inout float4 b, inout float4 c) {
    order(a, b);
    order(a, c);
}

void most3(inout float4 a, inout float4 b, inout float4 c) {
    order(b, c);
    order(a, c);
}

void extremes3(inout float4 a, inout float4 b, inout float4 c) {
    most3(a, b, c);
    order(a, b);
}

void extremes4(inout float4 a, inout float4 b, inout float4 c, inout float4 d) {
    order(a, b);
    order(c, d);
    order(a, c);
    order(b, d);
}

void extremes5(inout float4 a, inout float4 b, inout float4 c, inout float4 d, inout float4 e) {
    order(a, b);
    order(c, d);
    least3(a, c, e);
    most3(b, d, e);
}

void extremes6(inout float4 a, inout float4 b, inout float4 c, inout float4 d, inout float4 e, inout float4 f) {
    order(a, d);
    order(b, e);
    order(c, f);
    least3(a, b, c);
    most3(d, e, f);
}

float4 median9(float4 v[9]) {
    extremes6(v[0], v[1], v[2], v[3], v[4], v[5]);
    extremes5(v[1], v[2], v[3], v[4], v[6]);
    extremes4(v[2], v[3], v[4], v[7]);
    extremes3(v[3], v[4], v[8]);
    return v[4];
}

[shader("compute")]
[numthreads(8, 8, 1)]
void despeckle(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 center = rikka_effect_load(push.source, pixel);
    if (parameters.amount <= 0.0) {
        rikka_effect_store(push.target, pixel, center);
        return;
    }
    float4 window[9];
    for (int y = 0; y < 3; ++y) {
        for (int x = 0; x < 3; ++x) {
            window[y * 3 + x] = rikka_effect_load_clamped(push.source, pixel + int2(x - 1, y - 1), push.size);
        }
    }
    float4 middle = median9(window);
    float weight = clamp(parameters.amount, 0.0, 1.0);
    if (parameters.threshold > 0.0) {
        float reference = max(rikka_luma(max(middle.rgb, float3(0.0))), 1e-3);
        float contrast = abs(rikka_luma(center.rgb) - rikka_luma(middle.rgb)) / reference;
        weight *= smoothstep(0.5 * parameters.threshold, parameters.threshold, contrast);
    }
    rikka_effect_store(push.target, pixel, lerp(center, middle, weight));
}
