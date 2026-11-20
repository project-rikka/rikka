import rikka.effect;

struct Parameters {
    [Default(0.3)] [Range(-1.0, 1.0)] [Label("amount (barrel +, pincushion -)")] float amount;
    [Default(0.0)] [Range(-1.0, 1.0)] float cubic;
    [Default(0.5, 0.5)] [Range(0.0, 1.0)] float2 center;
    [Default(1.0)] [Range(0.0, 1.0)] float fit;
    [Default(0.0)] [Range(-1.0, 1.0)] [Label("dispersion (red outward +)")] float dispersion;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int WAVELENGTHS = 8;
static const float DISPERSION_SCALE = 0.03;

float3 spectral(float t) {
    return clamp(1.0 - abs(t - float3(0.85, 0.5, 0.15)) * 2.5, 0.0, 1.0);
}

float4 lookup(float2 source) {
    bool inside = all(source >= 0.0) && all(source <= 1.0);
    return inside ? rikka_effect_sample(push.source, source) : float4(0.0);
}

float distortion(Parameters parameters, float radius_squared) {
    float k1 = -0.5 * parameters.amount;
    float k2 = -0.25 * parameters.cubic;
    return 1.0 + k1 * radius_squared + k2 * radius_squared * radius_squared;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void warp(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float aspect = rikka_effect_aspect(push.size);
    float2 half_extent = float2(aspect, 1.0) * 0.5;
    float diagonal = length(half_extent);
    float shortest = min(half_extent.x, half_extent.y) / diagonal;
    float widest = max(
        max(distortion(parameters, 1.0), distortion(parameters, shortest * shortest)),
        distortion(parameters, (half_extent.x / diagonal) * (half_extent.x / diagonal)));
    float scale = lerp(1.0, 1.0 / max(widest, 1e-3), clamp(parameters.fit, 0.0, 1.0));
    float2 uv = rikka_effect_uv(pixel, push.size);
    float2 offset = (uv - parameters.center) * float2(aspect, 1.0) / diagonal;
    float radius_squared = dot(offset, offset);
    float2 bent = offset * distortion(parameters, radius_squared) * scale;
    float2 back = diagonal / float2(aspect, 1.0);
    if (parameters.dispersion == 0.0) {
        rikka_effect_store(push.target, pixel, lookup(parameters.center + bent * back));
        return;
    }
    float4 sum = float4(0.0);
    float4 total = float4(0.0);
    for (int wavelength = 0; wavelength < WAVELENGTHS; ++wavelength) {
        float t = (float(wavelength) + 0.5) / float(WAVELENGTHS);
        float magnification = 1.0 - parameters.dispersion * DISPERSION_SCALE * (t - 0.5) * 2.0 * radius_squared;
        float4 weight = float4(spectral(t), 1.0);
        sum += lookup(parameters.center + bent * magnification * back) * weight;
        total += weight;
    }
    rikka_effect_store(push.target, pixel, sum / total);
}
