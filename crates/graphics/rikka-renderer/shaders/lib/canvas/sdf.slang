static const float CORNER_REACH = 1.6;
static const float ARC_INSET = 0.29289322;
static const float FREE_EXPONENT = 3.42823956;

float sd_rounded_rect_within(float2 p, float2 half_size, float4 radii, float limit) {
    float r = min((p.x > 0.0) ? ((p.y > 0.0) ? radii.z : radii.y)
                              : ((p.y > 0.0) ? radii.w : radii.x),
                  limit);
    if (r <= 0.0) {
        float2 d = abs(p) - half_size;
        if (min(d.x, d.y) <= 0.0) {
            return max(d.x, d.y);
        }
        return length(d);
    }
    float reach = min(r * CORNER_REACH, limit);
    float2 d = abs(p) - half_size + reach;
    if (min(d.x, d.y) <= 0.0) {
        return max(d.x, d.y) - reach;
    }
    if (r >= limit) {
        return length(d) - reach;
    }
    float n = (reach < r * CORNER_REACH)
        ? -0.69314718 / log(1.0 - ARC_INSET * r / reach)
        : FREE_EXPONENT;
    float2 m = d / reach;
    float2 powered = pow(m, float2(n));
    float sum = powered.x + powered.y;
    float norm = pow(sum, 1.0 / n);
    float2 slope = (powered / m) * (norm / sum);
    return (norm - 1.0) * reach / length(slope);
}

float sd_rounded_rect(float2 p, float2 half_size, float4 radii) {
    return sd_rounded_rect_within(p, half_size, radii, min(half_size.x, half_size.y));
}

float2 bevel_normal(float2 p, float2 half_size) {
    float2 inv = 1.0 / max(half_size, float2(1.0e-6));
    float2 q = p * inv;
    float2 g = q * q * q * inv;
    float len = length(g);
    return (len > 1.0e-9) ? g / len : float2(0.0, -1.0);
}

float bevel_displacement(float t, float bending) {
    float u = 1.0 - clamp(t, 0.0, 1.0);
    float tangent = u * rsqrt(max(1.0 - u * u, 4.0e-6));
    return min(tangent * bending, 1.0) * u * u;
}
