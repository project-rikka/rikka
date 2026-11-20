[[vk::binding(4, 0)]] StructuredBuffer<uint> curve_words;

static const uint CURVE_BOUNDS_WORD = 1u;
static const uint CURVE_BAND_HEADERS_WORD = 5u;
static const uint CURVE_HORIZONTAL_BANDS_SHIFT = 16u;
static const uint CURVE_VERTICAL_BANDS_SHIFT = 24u;
static const uint CURVE_BAND_OFFSET_MASK = 0xFFFFu;
static const uint CURVE_BAND_COUNT_SHIFT = 16u;
static const float CURVE_INV_UNORM_MAX = 1.0 / 65535.0;
static const uint CURVE_ROOTS_UP = 0x74u;
static const uint CURVE_ROOTS_DOWN = 0x2Eu;
static const float CURVE_MIN_WEIGHT = 1.0 / 65536.0;

struct CurveRay {
    float coverage;
    float weight;

    [mutating]
    void cross(float along, float direction) {
        coverage += direction * clamp(along + 0.5, 0.0, 1.0);
        weight = max(weight, clamp(1.0 - 2.0 * abs(along), 0.0, 1.0));
    }
}

uint curve_band(float value, float low, float high, uint count) {
    float span = high - low;
    if (span <= 0.0 || count <= 1u) {
        return 0u;
    }
    float position = floor((value - low) / span * float(count));
    return uint(clamp(position, 0.0, float(count - 1u)));
}

float curve_divide(float numerator, float denominator) {
    return denominator == 0.0 ? 0.0 : numerator / denominator;
}

float curve_unit(float t) {
    return t >= 0.0 ? min(t, 1.0) : 0.0;
}

float2 curve_point(uint word, float2 origin, float2 scale) {
    float2 unorm = float2(float(word & 0xFFFFu), float(word >> 16));
    return origin + unorm * scale;
}

CurveRay curve_cast(uint base, uint band_word, float2 origin, float2 scale, bool vertical) {
    CurveRay ray = { 0.0, 0.0 };
    uint band = curve_words[base + band_word];
    uint offset = band & CURVE_BAND_OFFSET_MASK;
    uint count = band >> CURVE_BAND_COUNT_SHIFT;
    float sign_of_ray = vertical ? -1.0 : 1.0;
    uint references = 0u;
    for (uint index = 0u; index < count; ++index) {
        if ((index & 1u) == 0u) {
            references = curve_words[base + offset + (index >> 1u)];
        }
        uint curve = (references >> ((index & 1u) << 4u)) & 0xFFFFu;
        uint at = base + curve;
        float2 p0 = curve_point(curve_words[at], origin, scale);
        float2 p1 = curve_point(curve_words[at + 1u], origin, scale);
        float2 p2 = curve_point(curve_words[at + 2u], origin, scale);
        float3 along = vertical ? float3(p0.y, p1.y, p2.y) : float3(p0.x, p1.x, p2.x);
        float3 across = vertical ? float3(p0.x, p1.x, p2.x) : float3(p0.y, p1.y, p2.y);
        if (max(max(along.x, along.y), along.z) < -0.5) {
            break;
        }
        uint code = uint(across.x > 0.0) | (uint(across.y > 0.0) << 1) | (uint(across.z > 0.0) << 2);
        bool up = ((CURVE_ROOTS_UP >> code) & 1u) == 1u;
        bool down = ((CURVE_ROOTS_DOWN >> code) & 1u) == 1u;
        if (!up && !down) {
            continue;
        }
        float a = across.x - 2.0 * across.y + across.z;
        float b = across.x - across.y;
        float root = sqrt(max(b * b - a * across.x, 0.0));
        float quadratic = along.x - 2.0 * along.y + along.z;
        float linear = along.x - along.y;
        if (b >= 0.0) {
            float q = b + root;
            if (up) {
                float t = curve_unit(curve_divide(q, a));
                ray.cross((quadratic * t - 2.0 * linear) * t + along.x, sign_of_ray);
            }
            if (down) {
                float t = curve_unit(curve_divide(across.x, q));
                ray.cross((quadratic * t - 2.0 * linear) * t + along.x, -sign_of_ray);
            }
        } else {
            float q = b - root;
            if (up) {
                float t = curve_unit(curve_divide(across.x, q));
                ray.cross((quadratic * t - 2.0 * linear) * t + along.x, sign_of_ray);
            }
            if (down) {
                float t = curve_unit(curve_divide(q, a));
                ray.cross((quadratic * t - 2.0 * linear) * t + along.x, -sign_of_ray);
            }
        }
    }
    return ray;
}

float glyph_curve_coverage(uint base, float2 pixel, float pixels_per_em) {
    uint counts = curve_words[base];
    uint b = base + CURVE_BOUNDS_WORD;
    float4 bounds = asfloat(uint4(curve_words[b], curve_words[b + 1u], curve_words[b + 2u],
                                  curve_words[b + 3u]));
    uint horizontal_bands = (counts >> CURVE_HORIZONTAL_BANDS_SHIFT) & 0xFFu;
    uint vertical_bands = (counts >> CURVE_VERTICAL_BANDS_SHIFT) & 0xFFu;
    float2 origin = (bounds.xy - pixel) * pixels_per_em;
    float2 scale = (bounds.zw - bounds.xy) * (pixels_per_em * CURVE_INV_UNORM_MAX);
    CurveRay horizontal = curve_cast(
        base, CURVE_BAND_HEADERS_WORD + curve_band(pixel.y, bounds.y, bounds.w, horizontal_bands),
        origin, scale, false);
    CurveRay vertical = curve_cast(
        base,
        CURVE_BAND_HEADERS_WORD + horizontal_bands
            + curve_band(pixel.x, bounds.x, bounds.z, vertical_bands),
        origin, scale, true);
    float total = horizontal.weight + vertical.weight;
    float blended = abs(horizontal.coverage * horizontal.weight
                        + vertical.coverage * vertical.weight)
        / max(total, CURVE_MIN_WEIGHT);
    float inside = min(abs(horizontal.coverage), abs(vertical.coverage));
    return clamp(max(blended, inside), 0.0, 1.0);
}
