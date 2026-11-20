__exported import rikka.effects.blur;

struct Parameters {
    [Default(8.0)] [Range(1.0, 16.0)] [Label("radius (px at 1080p)")] float radius;
    [Default(8.0)] [Range(1.0, 18.0)] float sharpness;
    [Default(1.0)] [Range(0.1, 4.0)] float anisotropy;
    [Default(2.0)] [Range(0.5, 6.0)] [Label("orientation smoothing (px at 1080p)")] float smoothing;
    [Default(0.58)] [Range(0.1, 1.5)] [Hint("advanced")] float zero_crossing;
};

extension Parameters : IBlurParameters {
    static float sigma(Parameters* parameters, uint2 size) {
        return parameters.smoothing * rikka_effect_scale(size);
    }
}

static const int RIKKA_KUWAHARA_SECTORS = 8;
static const float RIKKA_KUWAHARA_MAX_RADIUS = 24.0;
static const float RIKKA_KUWAHARA_SPACING = 3.0;
static const float RIKKA_KUWAHARA_STEPS = 4.0;

float4 rikka_structure_tensor(uint source, int2 pixel, uint2 size) {
    float3 a = rikka_effect_straight(rikka_effect_load_clamped(source, pixel + int2(-1, -1), size));
    float3 b = rikka_effect_straight(rikka_effect_load_clamped(source, pixel + int2(0, -1), size));
    float3 c = rikka_effect_straight(rikka_effect_load_clamped(source, pixel + int2(1, -1), size));
    float3 d = rikka_effect_straight(rikka_effect_load_clamped(source, pixel + int2(-1, 0), size));
    float3 f = rikka_effect_straight(rikka_effect_load_clamped(source, pixel + int2(1, 0), size));
    float3 g = rikka_effect_straight(rikka_effect_load_clamped(source, pixel + int2(-1, 1), size));
    float3 h = rikka_effect_straight(rikka_effect_load_clamped(source, pixel + int2(0, 1), size));
    float3 i = rikka_effect_straight(rikka_effect_load_clamped(source, pixel + int2(1, 1), size));
    float3 gx = (c + 2.0 * f + i - a - 2.0 * d - g) * 0.25;
    float3 gy = (g + 2.0 * h + i - a - 2.0 * b - c) * 0.25;
    return float4(dot(gx, gx), dot(gy, gy), dot(gx, gy), 1.0);
}

float4 rikka_kuwahara(Parameters parameters, uint source, uint tensor, int2 pixel, uint2 size) {
    float2 uv = rikka_effect_uv(pixel, size);
    float4 structure = rikka_effect_sample(tensor, uv);
    float e = structure.x;
    float g = structure.y;
    float f = structure.z;
    float root = sqrt(max((e - g) * (e - g) + 4.0 * f * f, 0.0));
    float major = 0.5 * (e + g + root);
    float minor = 0.5 * (e + g - root);
    float2 direction = float2(major - e, -f);
    direction = length(direction) > 1e-8 ? normalize(direction) : float2(0.0, 1.0);
    float phi = -atan2(direction.y, direction.x);
    float strength = major + minor > 1e-8 ? (major - minor) / (major + minor) : 0.0;

    float radius = clamp(parameters.radius * rikka_effect_scale(size), 1.0, RIKKA_KUWAHARA_MAX_RADIUS);
    float alpha = max(parameters.anisotropy, 0.05);
    float a = radius * clamp((alpha + strength) / alpha, 0.1, 2.0);
    float b = radius * clamp(alpha / (alpha + strength), 0.1, 2.0);
    float cos_phi = cos(phi);
    float sin_phi = sin(phi);
    int steps = int(clamp(ceil(a / RIKKA_KUWAHARA_SPACING), 2.0, RIKKA_KUWAHARA_STEPS));
    float2 axis_major = float2(cos_phi, -sin_phi) * (2.0 * a);
    float2 axis_minor = float2(sin_phi, cos_phi) * (2.0 * b);
    float2 texel = 1.0 / float2(size);

    float zeta = 1.0 / radius;
    float crossing = parameters.zero_crossing;
    float eta = (zeta + cos(crossing)) / (sin(crossing) * sin(crossing));

    float4 m[8];
    float3 s[8];
    for (int k = 0; k < RIKKA_KUWAHARA_SECTORS; ++k) {
        m[k] = float4(0.0);
        s[k] = float3(0.0);
    }
    float4 centre = rikka_effect_load(source, pixel);
    for (int y = -steps; y <= steps; ++y) {
        for (int x = -steps; x <= steps; ++x) {
            float2 v = float2(float(x), float(y)) / float(2 * steps);
            if (dot(v, v) > 0.25) {
                continue;
            }
            float2 offset = axis_major * v.x + axis_minor * v.y;
            float3 c = clamp(rikka_effect_straight(rikka_effect_sample(source, uv + offset * texel)), 0.0, 1.0);
            float w[8];
            float sum = 0.0;
            float vxx = zeta - eta * v.x * v.x;
            float vyy = zeta - eta * v.y * v.y;
            float z = max(0.0, v.y + vxx);
            w[0] = z * z;
            z = max(0.0, -v.x + vyy);
            w[2] = z * z;
            z = max(0.0, -v.y + vxx);
            w[4] = z * z;
            z = max(0.0, v.x + vyy);
            w[6] = z * z;
            float2 r = 0.70710678 * float2(v.x - v.y, v.x + v.y);
            vxx = zeta - eta * r.x * r.x;
            vyy = zeta - eta * r.y * r.y;
            z = max(0.0, r.y + vxx);
            w[1] = z * z;
            z = max(0.0, -r.x + vyy);
            w[3] = z * z;
            z = max(0.0, -r.y + vxx);
            w[5] = z * z;
            z = max(0.0, r.x + vyy);
            w[7] = z * z;
            for (int k = 0; k < RIKKA_KUWAHARA_SECTORS; ++k) {
                sum += w[k];
            }
            float gaussian = exp(-3.125 * dot(v, v)) / max(sum, 1e-8);
            for (int k = 0; k < RIKKA_KUWAHARA_SECTORS; ++k) {
                float weight = w[k] * gaussian;
                m[k] += float4(c * weight, weight);
                s[k] += c * c * weight;
            }
        }
    }
    float4 result = float4(0.0);
    for (int k = 0; k < RIKKA_KUWAHARA_SECTORS; ++k) {
        if (m[k].w <= 1e-8) {
            continue;
        }
        float3 mean = m[k].rgb / m[k].w;
        float3 variance = abs(s[k] / m[k].w - mean * mean);
        float sigma = variance.r + variance.g + variance.b;
        float weight = 1.0 / (1.0 + pow(255.0 * sigma, 0.5 * parameters.sharpness));
        result += float4(mean * weight, weight);
    }
    float3 painted = result.w > 1e-8 ? result.rgb / result.w : rikka_effect_straight(centre);
    return float4(painted * centre.a, centre.a);
}
