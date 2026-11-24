static const float SHADOW_REACH = 3.0;
static const uint SHADOW_SAMPLES = 4u;

float2 shadow_erf(float2 x) {
    float2 s = sign(x);
    float2 a = abs(x);
    float2 r = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
    r *= r;
    return s - s / (r * r);
}

float shadow_gaussian(float x, float sigma) {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (2.50662827 * sigma);
}

float shadow_row(float x, float y, float sigma, float corner, float2 half_size) {
    float delta = min(half_size.y - corner - abs(y), 0.0);
    float curved = half_size.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    float2 integral = 0.5 + 0.5 * shadow_erf((x + float2(-curved, curved)) * (0.70710678 / sigma));
    return integral.y - integral.x;
}

float rounded_box_shadow(float2 p, float2 half_size, float4 radii, float sigma) {
    float corner = (p.x > 0.0) ? ((p.y > 0.0) ? radii.z : radii.y)
                               : ((p.y > 0.0) ? radii.w : radii.x);
    corner = min(corner, min(half_size.x, half_size.y));
    float low = p.y - half_size.y;
    float high = p.y + half_size.y;
    float start = clamp(-SHADOW_REACH * sigma, low, high);
    float end = clamp(SHADOW_REACH * sigma, low, high);
    float step = (end - start) / float(SHADOW_SAMPLES);
    float y = start + step * 0.5;
    float value = 0.0;
    for (uint i = 0u; i < SHADOW_SAMPLES; i++) {
        value += shadow_row(p.x, p.y - y, sigma, corner, half_size) * shadow_gaussian(y, sigma) * step;
        y += step;
    }
    return value;
}
