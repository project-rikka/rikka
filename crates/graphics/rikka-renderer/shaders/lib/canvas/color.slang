struct Conversion {
    uint4 mode;
    float4 decode;
    float4 row0;
    float4 row1;
    float4 row2;
    float4 tone;
}

static const uint DECODE_IDENTITY = 0u;
static const uint DECODE_POWER = 1u;
static const uint DECODE_COMPOUND_POWER = 2u;
static const uint DECODE_PQ = 3u;
static const uint DECODE_HLG = 4u;
static const uint DECODE_LINEAR = 5u;

static const uint ALPHA_PREMULTIPLIED = 0u;
static const uint ALPHA_OPTICAL = 1u;

static const uint ENCODE_LINEAR = 1u;

static const float PQ_M1 = float(2610.0l / 16384.0l);
static const float PQ_M2 = float(2523.0l / 4096.0l * 128.0l);
static const float PQ_INV_M1 = float(16384.0l / 2610.0l);
static const float PQ_INV_M2 = float(4096.0l / (2523.0l * 128.0l));
static const float PQ_C1 = float(3424.0l / 4096.0l);
static const float PQ_C2 = float(2413.0l / 4096.0l * 32.0l);
static const float PQ_C3 = float(2392.0l / 4096.0l * 32.0l);

static const float HLG_A = 0.17883277;
static const float HLG_B = 0.28466892;
static const float HLG_C = 0.55991073;

static const float GAMMA22_INVERSE = float(1.0l / 2.2l);

float3 signed_pow3(float3 value, float exponent) {
    float3 raised = pow(abs(value), float3(exponent));
    return select(value < float3(0.0), -raised, raised);
}

float3 compound_power3(float3 value) {
    float3 magnitude = abs(value);
    float3 low = magnitude * (1.0 / 12.92);
    float3 high = pow((magnitude + 0.055) * (1.0 / 1.055), float3(2.4));
    float3 decoded = select(magnitude <= float3(0.04045), low, high);
    return select(value < float3(0.0), -decoded, decoded);
}

float pq_decode(float encoded) {
    float raised = pow(clamp(encoded, 0.0, 1.0), PQ_INV_M2);
    return pow(max(raised - PQ_C1, 0.0) / (PQ_C2 - PQ_C3 * raised), PQ_INV_M1);
}

float3 pq_decode3(float3 encoded) {
    float3 raised = pow(clamp(encoded, float3(0.0), float3(1.0)), float3(PQ_INV_M2));
    return pow(max(raised - PQ_C1, float3(0.0)) / (PQ_C2 - PQ_C3 * raised), float3(PQ_INV_M1));
}

float pq_encode(float light) {
    float raised = pow(clamp(light, 0.0, 1.0), PQ_M1);
    return pow((PQ_C1 + PQ_C2 * raised) / (1.0 + PQ_C3 * raised), PQ_M2);
}

float3 pq_encode3(float3 light) {
    float3 raised = pow(clamp(light, float3(0.0), float3(1.0)), float3(PQ_M1));
    return pow((PQ_C1 + PQ_C2 * raised) / (1.0 + PQ_C3 * raised), float3(PQ_M2));
}

float3 hlg_decode3(float3 encoded) {
    float3 signal = clamp(encoded, float3(0.0), float3(1.0));
    float3 low = signal * signal * (1.0 / 3.0);
    float3 high = (exp((signal - HLG_C) * (1.0 / HLG_A)) + HLG_B) * (1.0 / 12.0);
    return select(signal <= float3(0.5), low, high);
}

float3 decode_rgb(float3 encoded, Conversion conversion) {
    switch (conversion.mode.x) {
    case DECODE_LINEAR:
        return encoded;
    case DECODE_POWER:
        return signed_pow3(encoded, conversion.decode.x);
    case DECODE_COMPOUND_POWER:
        return compound_power3(encoded);
    case DECODE_PQ:
        return pq_decode3(encoded);
    default:
        return hlg_decode3(encoded);
    }
}

float eetf(float relative, Conversion conversion) {
    float anchor = conversion.tone.x;
    float source_peak = conversion.tone.y;
    float target_peak = conversion.tone.z;
    float knee = conversion.tone.w;
    float encoded = min(pq_encode(relative * anchor) / source_peak, 1.0);
    if (encoded <= knee) {
        return relative;
    }
    float span = 1.0 - knee;
    float t = (encoded - knee) / span;
    float t2 = t * t;
    float t3 = t2 * t;
    float mapped = (2.0 * t3 - 3.0 * t2 + 1.0) * knee + (t3 - 2.0 * t2 + t) * span
        + (-2.0 * t3 + 3.0 * t2) * target_peak;
    return pq_decode(mapped * source_peak) / anchor;
}

float4 convert(float4 sampled, Conversion conversion) {
    if (conversion.mode.x == DECODE_IDENTITY) {
        return sampled;
    }
    float alpha = sampled.a;
    if (alpha <= 0.0) {
        return float4(0.0);
    }
    float3 encoded = sampled.rgb;
    if (conversion.mode.y == ALPHA_PREMULTIPLIED) {
        encoded /= alpha;
    }
    float3 light = decode_rgb(encoded, conversion);
    if (conversion.mode.y == ALPHA_OPTICAL) {
        light /= alpha;
    }
    if (conversion.decode.w > 0.5) {
        float luminance = dot(float3(conversion.row0.w, conversion.row1.w, conversion.row2.w), light);
        light *= luminance > 0.0 ? pow(luminance, conversion.decode.z) : 0.0;
    }
    float3 color = float3(dot(conversion.row0.xyz, light), dot(conversion.row1.xyz, light),
                          dot(conversion.row2.xyz, light));
    if (conversion.mode.z != 0u) {
        float peak = max(color.r, max(color.g, color.b));
        if (peak > 0.0) {
            color *= eetf(peak, conversion) / peak;
        }
    }
    if (conversion.mode.w == ENCODE_LINEAR) {
        color = max(color, float3(0.0));
    } else {
        color = pow(clamp(color, float3(0.0), float3(1.0)), float3(GAMMA22_INVERSE));
    }
    return float4(color * alpha, alpha);
}
