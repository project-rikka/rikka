__exported import rikka.common;

static const uint POST_TONE_GT = 0u;
static const uint POST_TONE_AGX = 1u;
static const uint POST_TONE_ACES = 2u;
static const uint POST_TONE_NEUTRAL = 3u;
static const uint POST_TONE_CLAMP = 4u;
static const uint POST_TONE_HABLE = 5u;
static const uint POST_TONE_REINHARD = 6u;
static const uint POST_TONE_LOTTES = 7u;
static const uint POST_TONE_CUSTOM = 8u;

static const uint POST_AGX_BASE = 0u;
static const uint POST_AGX_PUNCHY = 1u;
static const uint POST_AGX_GOLDEN = 2u;

struct PostCurve {
    float peak;
    float contrast;
    float toe;
    float linear_length;
    float toe_power;
};

struct PostTone {
    uint tone;
    uint look;
    float preserve_hue;
    PostCurve curve;
};

float3 post_uchimura(float3 x, PostCurve curve) {
    float peak = max(curve.peak, 1e-3);
    float slope = max(curve.contrast, 1e-3);
    float toe_end = clamp(curve.toe, 1e-3, peak * 0.99);
    float linear_length = clamp(curve.linear_length, 0.0, 0.99);
    float toe_power = max(curve.toe_power, 1e-3);
    float linear_span = (peak - toe_end) * linear_length / slope;
    float shoulder_start = toe_end + linear_span;
    float shoulder_floor = toe_end + slope * linear_span;
    float shoulder_scale = slope * peak / max(peak - shoulder_floor, 1e-4);
    float shoulder_rate = -shoulder_scale / peak;
    float3 toe_weight = 1.0 - smoothstep(0.0, toe_end, x);
    float3 shoulder_weight = step(shoulder_start, x);
    float3 linear_weight = 1.0 - toe_weight - shoulder_weight;
    float3 toe = toe_end * pow(max(x, float3(0.0)) / toe_end, float3(toe_power));
    float3 shoulder = peak - (peak - shoulder_floor) * exp(shoulder_rate * (x - shoulder_start));
    float3 straight = toe_end + slope * (x - toe_end);
    return toe * toe_weight + straight * linear_weight + shoulder * shoulder_weight;
}

float3 post_gran_turismo(float3 x) {
    PostCurve curve;
    curve.peak = 1.0;
    curve.contrast = 1.0;
    curve.toe = 0.22;
    curve.linear_length = 0.4;
    curve.toe_power = 1.33;
    return post_uchimura(x, curve);
}

float3 post_agx_contrast(float3 x) {
    float3 x2 = x * x;
    float3 x4 = x2 * x2;
    return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2
        + 0.1191 * x - 0.00232;
}

float3 post_agx_look(uint look, float3 encoded) {
    float3 slope = float3(1.0);
    float3 power = float3(1.0);
    float saturation = 1.0;
    if (look == POST_AGX_PUNCHY) {
        power = float3(1.35);
        saturation = 1.4;
    } else if (look == POST_AGX_GOLDEN) {
        slope = float3(1.0, 0.9, 0.5);
        power = float3(0.8);
        saturation = 1.3;
    } else {
        return encoded;
    }
    float3 graded = pow(max(encoded * slope, float3(0.0)), power);
    float luma = rikka_luma(graded);
    return luma + saturation * (graded - luma);
}

float3 post_agx(float3 color, uint look) {
    const float3x3 inset = transpose(float3x3(
        0.842479062253094, 0.0423282422610123, 0.0423756549057051,
        0.0784335999999992, 0.878468636469772, 0.0784336,
        0.0792237451477643, 0.0791661274605434, 0.879142973793104));
    const float3x3 outset = transpose(float3x3(
        1.19687900512017, -0.0528968517574562, -0.0529716355144438,
        -0.0980208811401368, 1.15190312990417, -0.0980434501171241,
        -0.0990297440797205, -0.0989611768448433, 1.15107367264116));
    const float lowest = -12.47393;
    const float highest = 4.026069;
    float3 encoded = clamp(log2(max(mul(inset, color), float3(1e-10))), lowest, highest);
    encoded = post_agx_contrast((encoded - lowest) / (highest - lowest));
    encoded = post_agx_look(look, encoded);
    return pow(clamp(mul(outset, encoded), 0.0, 1.0), float3(2.2));
}

float3 post_aces(float3 color) {
    const float3x3 into = transpose(float3x3(
        0.59719, 0.07600, 0.02840,
        0.35458, 0.90834, 0.13383,
        0.04823, 0.01566, 0.83777));
    const float3x3 out_of = transpose(float3x3(
        1.60475, -0.10208, -0.00327,
        -0.53108, 1.10813, -0.07276,
        -0.07367, -0.00605, 1.07602));
    float3 v = mul(into, color);
    float3 a = v * (v + 0.0245786) - 0.000090537;
    float3 b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return clamp(mul(out_of, a / b), 0.0, 1.0);
}

float3 post_neutral(float3 color) {
    const float start = 0.8 - 0.04;
    const float desaturation = 0.15;
    float lowest = min(color.r, min(color.g, color.b));
    float offset = lowest < 0.08 ? lowest - 6.25 * lowest * lowest : 0.04;
    color -= offset;
    float peak = max(color.r, max(color.g, color.b));
    if (peak < start) {
        return color;
    }
    float d = 1.0 - start;
    float new_peak = 1.0 - d * d / (peak + d - start);
    color *= new_peak / peak;
    float g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
    return lerp(color, float3(new_peak), g);
}

float3 post_hable_partial(float3 x) {
    const float a = 0.15;
    const float b = 0.50;
    const float c = 0.10;
    const float d = 0.20;
    const float e = 0.02;
    const float f = 0.30;
    return (x * (a * x + c * b) + d * e) / (x * (a * x + b) + d * f) - e / f;
}

float3 post_hable(float3 color) {
    const float white = 11.2;
    const float bias = 2.0;
    return post_hable_partial(color * bias) / post_hable_partial(float3(white)).x;
}

float3 post_reinhard_jodie(float3 color) {
    float luma = rikka_luma(color);
    float3 compressed = color / (1.0 + color);
    return lerp(color / (1.0 + luma), compressed, compressed);
}

float3 post_lottes(float3 color) {
    const float contrast = 1.6;
    const float shoulder = 0.977;
    const float white = 8.0;
    const float middle_in = 0.18;
    const float middle_out = 0.267;
    float white_a = pow(white, contrast);
    float middle_a = pow(middle_in, contrast);
    float white_ad = pow(white_a, shoulder);
    float middle_ad = pow(middle_a, shoulder);
    float denominator = (white_ad - middle_ad) * middle_out;
    float b = (-middle_a + white_a * middle_out) / denominator;
    float c = (white_ad * middle_a - white_a * middle_ad * middle_out) / denominator;
    float3 x = max(color, float3(0.0));
    return pow(x, float3(contrast)) / (pow(x, float3(contrast * shoulder)) * b + c);
}

float3 post_curve(PostTone tone, float3 color) {
    switch (tone.tone) {
        case POST_TONE_AGX:
            return post_agx(color * 1.25, tone.look);
        case POST_TONE_ACES:
            return post_aces(color * 1.6);
        case POST_TONE_NEUTRAL:
            return clamp(post_neutral(color), 0.0, 1.0);
        case POST_TONE_CLAMP:
            return clamp(color, 0.0, 1.0);
        case POST_TONE_HABLE:
            return clamp(post_hable(color), 0.0, 1.0);
        case POST_TONE_REINHARD:
            return clamp(post_reinhard_jodie(max(color, float3(0.0))), 0.0, 1.0);
        case POST_TONE_LOTTES:
            return clamp(post_lottes(color), 0.0, 1.0);
        case POST_TONE_CUSTOM:
            return clamp(post_uchimura(color, tone.curve), 0.0, 1.0);
        default:
            return clamp(post_gran_turismo(color), 0.0, 1.0);
    }
}

float3 post_hue_kept(PostTone tone, float3 color) {
    float luma = rikka_luma(max(color, float3(0.0)));
    if (luma <= 1e-6) {
        return post_curve(tone, color);
    }
    float mapped = rikka_luma(post_curve(tone, float3(luma)));
    float3 kept = max(color, float3(0.0)) * (mapped / luma);
    float brightest = max(kept.r, max(kept.g, kept.b));
    if (brightest > 1.0 && mapped < 1.0) {
        float t = (brightest - 1.0) / max(brightest - mapped, 1e-6);
        kept = lerp(kept, float3(mapped), clamp(t, 0.0, 1.0));
    }
    return clamp(kept, 0.0, 1.0);
}

float3 post_tone_map(PostTone tone, float3 color) {
    float3 mapped = post_curve(tone, color);
    float preserve = clamp(tone.preserve_hue, 0.0, 1.0);
    if (preserve <= 0.0) {
        return mapped;
    }
    return lerp(mapped, post_hue_kept(tone, color), preserve);
}
