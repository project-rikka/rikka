import floating;

float lum(float3 c) {
    return dot(c, float3(0.3, 0.59, 0.11));
}

float3 clip_color(float3 c) {
    float l = lum(c);
    float n = min(c.r, min(c.g, c.b));
    float x = max(c.r, max(c.g, c.b));
    if (n < 0.0) {
        c = l + (c - l) * l / max(l - n, 1e-9);
    }
    if (x > 1.0) {
        c = l + (c - l) * (1.0 - l) / max(x - l, 1e-9);
    }
    return clamp(c, 0.0, 1.0);
}

float3 set_lum(float3 c, float l) {
    return clip_color(c + (l - lum(c)));
}

float sat(float3 c) {
    return max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
}

float3 set_sat(float3 c, float s) {
    float high = max(c.r, max(c.g, c.b));
    float low = min(c.r, min(c.g, c.b));
    if (high <= low) {
        return float3(0.0);
    }
    return (c - low) * s / (high - low);
}

float3 rgb_to_hsl(float3 c) {
    float high = max(c.r, max(c.g, c.b));
    float low = min(c.r, min(c.g, c.b));
    float l = (high + low) * 0.5;
    float delta = high - low;
    if (delta <= 0.0) {
        return float3(0.0, 0.0, l);
    }
    float s = min(delta / max(1.0 - abs(2.0 * l - 1.0), 1e-12), 1.0);
    float h;
    if (high == c.r) {
        h = 60.0 * floored_mod((c.g - c.b) / delta, 6.0);
    } else if (high == c.g) {
        h = 60.0 * ((c.b - c.r) / delta + 2.0);
    } else {
        h = 60.0 * ((c.r - c.g) / delta + 4.0);
    }
    return float3(h, s, l);
}

float3 hsl_to_rgb(float3 hsl) {
    float c = (1.0 - abs(2.0 * hsl.z - 1.0)) * hsl.y;
    float h = floored_mod(hsl.x, 360.0) / 60.0;
    float x = c * (1.0 - abs(floored_mod(h, 2.0) - 1.0));
    float m = hsl.z - c * 0.5;
    float3 rgb;
    if (h < 1.0) {
        rgb = float3(c, x, 0.0);
    } else if (h < 2.0) {
        rgb = float3(x, c, 0.0);
    } else if (h < 3.0) {
        rgb = float3(0.0, c, x);
    } else if (h < 4.0) {
        rgb = float3(0.0, x, c);
    } else if (h < 5.0) {
        rgb = float3(x, 0.0, c);
    } else {
        rgb = float3(c, 0.0, x);
    }
    return rgb + m;
}

float srgb_to_linear(float v) {
    float a = abs(v);
    float l = a <= 0.04045 ? a / 12.92 : pow((a + 0.055) / 1.055, 2.4);
    return float_sign(v) * l;
}

float linear_to_srgb(float v) {
    float a = abs(v);
    float e = a <= 0.0031308 ? a * 12.92 : 1.055 * pow(a, float(1.0l / 2.4l)) - 0.055;
    return float_sign(v) * e;
}

static const uint TRANSFER_IDENTITY = 0u;
static const uint TRANSFER_SRGB = 1u;
static const uint TRANSFER_TABLES = 2u;
static const uint TABLE_SIZE = 4096u;

float table_sample(float* table, uint offset, float x) {
    float position = clamp(x, 0.0, 1.0) * float(TABLE_SIZE - 1u);
    uint low = min(uint(position), TABLE_SIZE - 2u);
    float t = position - float(low);
    return lerp(table[offset + low], table[offset + low + 1u], t);
}

float3 to_linear(float3 c, uint transfer, float* tables) {
    if (transfer == TRANSFER_SRGB) {
        return float3(srgb_to_linear(c.r), srgb_to_linear(c.g), srgb_to_linear(c.b));
    }
    if (transfer == TRANSFER_TABLES) {
        return float3(
            table_sample(tables, 0u, c.r),
            table_sample(tables, TABLE_SIZE, c.g),
            table_sample(tables, 2u * TABLE_SIZE, c.b));
    }
    return c;
}

float3 from_linear(float3 c, uint transfer, float* tables) {
    if (transfer == TRANSFER_SRGB) {
        return float3(linear_to_srgb(c.r), linear_to_srgb(c.g), linear_to_srgb(c.b));
    }
    if (transfer == TRANSFER_TABLES) {
        float3 root = sqrt(max(c, 0.0));
        return float3(
            table_sample(tables, 3u * TABLE_SIZE, root.r),
            table_sample(tables, 4u * TABLE_SIZE, root.g),
            table_sample(tables, 5u * TABLE_SIZE, root.b));
    }
    return c;
}

float hard_light(float cb, float cs) {
    if (cs <= 0.5) {
        return cb * 2.0 * cs;
    }
    float s = 2.0 * cs - 1.0;
    return cb + s - cb * s;
}

float separable(uint mode, float cb, float cs) {
    switch (mode) {
    case 1u: return cb * cs;
    case 2u: return cb + cs - cb * cs;
    case 3u: return hard_light(cs, cb);
    case 4u: return min(cb, cs);
    case 5u: return max(cb, cs);
    case 6u:
        if (cb <= 0.0) return 0.0;
        if (cs >= 1.0) return 1.0;
        return min(cb / (1.0 - cs), 1.0);
    case 7u:
        if (cb >= 1.0) return 1.0;
        if (cs <= 0.0) return 0.0;
        return 1.0 - min((1.0 - cb) / cs, 1.0);
    case 8u: return min(cb + cs, 1.0);
    case 9u: return max(cb + cs - 1.0, 0.0);
    case 10u: return hard_light(cb, cs);
    case 11u:
        if (cs <= 0.5) {
            return cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb);
        } else {
            float d = cb <= 0.25 ? ((16.0 * cb - 12.0) * cb + 4.0) * cb : sqrt(cb);
            return cb + (2.0 * cs - 1.0) * (d - cb);
        }
    case 12u:
        if (cs <= 0.0) return 0.0;
        if (cs >= 1.0) return 1.0;
        if (cs <= 0.5) return 1.0 - min((1.0 - cb) / (2.0 * cs), 1.0);
        return min(cb / (2.0 * (1.0 - cs)), 1.0);
    case 13u: return clamp(cb + 2.0 * cs - 1.0, 0.0, 1.0);
    case 14u: return cs > 0.5 ? max(cb, 2.0 * cs - 1.0) : min(cb, 2.0 * cs);
    case 15u: {
        float total = cb + cs;
        return (total > 1.0 || (total == 1.0 && cb > 0.5)) ? 1.0 : 0.0;
    }
    case 16u: return abs(cb - cs);
    case 17u: return cb + cs - 2.0 * cb * cs;
    case 18u: return max(cb - cs, 0.0);
    case 19u:
        if (cs <= 0.0) return cb <= 0.0 ? 0.0 : 1.0;
        return min(cb / cs, 1.0);
    default: return cs;
    }
}

float3 blended(uint mode, float3 cb, float3 cs) {
    switch (mode) {
    case 0u: return cs;
    case 20u: return lum(cs) < lum(cb) ? cs : cb;
    case 21u: return lum(cs) > lum(cb) ? cs : cb;
    case 22u: return set_lum(set_sat(cs, sat(cb)), lum(cb));
    case 23u: return set_lum(set_sat(cb, sat(cs)), lum(cb));
    case 24u: return set_lum(cs, lum(cb));
    case 25u: return set_lum(cb, lum(cs));
    default: {
        float3 b = clamp(cb, 0.0, 1.0);
        float3 s = clamp(cs, 0.0, 1.0);
        return float3(separable(mode, b.r, s.r), separable(mode, b.g, s.g), separable(mode, b.b, s.b));
    }
    }
}
