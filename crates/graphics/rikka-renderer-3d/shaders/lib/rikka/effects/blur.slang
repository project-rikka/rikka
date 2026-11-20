__exported import rikka.effect;

static const int RIKKA_BLUR_MAX_REACH = 64;

interface IBlurParameters {
    static float sigma(Ptr<This> parameters, uint2 size);

    static void store(uint image, int2 pixel, float4 value) {
        rikka_effect_store(image, pixel, value);
    }
}

struct RikkaBlurPush<P : IBlurParameters> {
    P* parameters;
    uint source;
    uint target;
    uint2 size;

    void spread(uint3 id, float2 axis) {
        int2 pixel = int2(id.xy);
        if (rikka_effect_outside(id.xy, size)) {
            return;
        }
        float sigma = P.sigma(parameters, size);
        P.store(target, pixel, rikka_blur_gaussian(source, pixel, size, axis, sigma));
    }
};

float4 rikka_blur_gaussian(uint image, int2 pixel, uint2 size, float2 axis, float sigma) {
    float2 texel = axis / float2(size);
    float2 uv = rikka_effect_uv(pixel, size);
    sigma = clamp(sigma, 0.3, float(RIKKA_BLUR_MAX_REACH) / 3.0);
    int reach = int(ceil(sigma * 3.0));
    float falloff = -0.5 / (sigma * sigma);
    float4 sum = rikka_effect_sample(image, uv);
    float total = 1.0;
    for (int at = 1; at <= reach; at += 2) {
        float near = exp(falloff * float(at * at));
        float far = exp(falloff * float((at + 1) * (at + 1)));
        float weight = near + far;
        float offset = (float(at) * near + float(at + 1) * far) / weight;
        float2 shift = texel * offset;
        sum += (rikka_effect_sample(image, uv + shift) + rikka_effect_sample(image, uv - shift)) * weight;
        total += 2.0 * weight;
    }
    return sum / total;
}

float4 rikka_blur_reduce(uint image, int2 pixel, uint2 size) {
    float2 uv = rikka_effect_uv(pixel, size);
    float2 quarter = 0.25 / float2(max(size, uint2(1u)));
    return 0.25 * (rikka_effect_sample(image, uv + float2(-quarter.x, -quarter.y))
        + rikka_effect_sample(image, uv + float2(quarter.x, -quarter.y))
        + rikka_effect_sample(image, uv + float2(-quarter.x, quarter.y))
        + rikka_effect_sample(image, uv + float2(quarter.x, quarter.y)));
}
