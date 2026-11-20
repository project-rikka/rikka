import rikka.common;

uint3 rikka_graph_pcg3(uint3 v) {
    v = v * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v ^= v >> 16u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return v;
}

float3 rikka_graph_hash3(float3 cell) {
    return float3(rikka_graph_pcg3(uint3(int3(cell)))) * (1.0 / 4294967295.0);
}

float rikka_graph_hash(float3 cell) {
    return rikka_graph_hash3(cell).x;
}

float rikka_graph_value_noise(float3 p) {
    float3 i = floor(p);
    float3 f = p - i;
    float3 u = f * f * (3.0 - 2.0 * f);
    float x00 = lerp(rikka_graph_hash(i), rikka_graph_hash(i + float3(1.0, 0.0, 0.0)), u.x);
    float x10 = lerp(rikka_graph_hash(i + float3(0.0, 1.0, 0.0)), rikka_graph_hash(i + float3(1.0, 1.0, 0.0)), u.x);
    float x01 = lerp(rikka_graph_hash(i + float3(0.0, 0.0, 1.0)), rikka_graph_hash(i + float3(1.0, 0.0, 1.0)), u.x);
    float x11 = lerp(rikka_graph_hash(i + float3(0.0, 1.0, 1.0)), rikka_graph_hash(i + float3(1.0, 1.0, 1.0)), u.x);
    return lerp(lerp(x00, x10, u.y), lerp(x01, x11, u.y), u.z);
}

float rikka_graph_corner(float3 i, float3 f, float3 corner) {
    float3 gradient = normalize(rikka_graph_hash3(i + corner) * 2.0 - 1.0 + 1e-6);
    return dot(gradient, f - corner);
}

float rikka_graph_gradient_noise(float3 p) {
    float3 i = floor(p);
    float3 f = p - i;
    float3 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    float x00 = lerp(rikka_graph_corner(i, f, float3(0.0, 0.0, 0.0)), rikka_graph_corner(i, f, float3(1.0, 0.0, 0.0)), u.x);
    float x10 = lerp(rikka_graph_corner(i, f, float3(0.0, 1.0, 0.0)), rikka_graph_corner(i, f, float3(1.0, 1.0, 0.0)), u.x);
    float x01 = lerp(rikka_graph_corner(i, f, float3(0.0, 0.0, 1.0)), rikka_graph_corner(i, f, float3(1.0, 0.0, 1.0)), u.x);
    float x11 = lerp(rikka_graph_corner(i, f, float3(0.0, 1.0, 1.0)), rikka_graph_corner(i, f, float3(1.0, 1.0, 1.0)), u.x);
    return lerp(lerp(x00, x10, u.y), lerp(x01, x11, u.y), u.z) * 0.5 + 0.5;
}

float2 rikka_graph_voronoi(float3 p, float randomness) {
    float3 i = floor(p);
    float3 f = p - i;
    float nearest = 8.0;
    float cell = 0.0;
    for (int z = -1; z <= 1; z++) {
        for (int y = -1; y <= 1; y++) {
            for (int x = -1; x <= 1; x++) {
                float3 offset = float3(float(x), float(y), float(z));
                float3 jitter = rikka_graph_hash3(i + offset);
                float gap = length(offset + jitter * randomness - f);
                if (gap < nearest) {
                    nearest = gap;
                    cell = jitter.y;
                }
            }
        }
    }
    return float2(nearest, cell);
}

float rikka_graph_fbm_value(float3 p, float octaves, float lacunarity, float gain) {
    int count = clamp(int(octaves), 1, 8);
    float sum = 0.0;
    float amplitude = 1.0;
    float total = 0.0;
    for (int octave = 0; octave < count; octave++) {
        sum += rikka_graph_value_noise(p) * amplitude;
        total += amplitude;
        p *= lacunarity;
        amplitude *= gain;
    }
    return sum / max(total, 1e-6);
}

float rikka_graph_fbm_gradient(float3 p, float octaves, float lacunarity, float gain) {
    int count = clamp(int(octaves), 1, 8);
    float sum = 0.0;
    float amplitude = 1.0;
    float total = 0.0;
    for (int octave = 0; octave < count; octave++) {
        sum += rikka_graph_gradient_noise(p) * amplitude;
        total += amplitude;
        p *= lacunarity;
        amplitude *= gain;
    }
    return sum / max(total, 1e-6);
}

float2 rikka_graph_rotate(float2 uv, float2 center, float angle) {
    float s = sin(angle);
    float c = cos(angle);
    float2 d = uv - center;
    return center + float2(c * d.x - s * d.y, s * d.x + c * d.y);
}

float2 rikka_graph_polar(float2 uv, float2 center) {
    float2 d = uv - center;
    return float2(length(d), atan2(d.y, d.x) / RIKKA_TAU + 0.5);
}

float3 rikka_graph_bounded(float3 offset, float reach) {
    float distance = length(offset);
    return reach >= 0.0 && distance > reach ? offset * (reach / distance) : offset;
}

float3 rikka_graph_to_tangent(float3 v, float3 normal, float4 tangent) {
    float3 t = normalize(tangent.xyz - normal * dot(normal, tangent.xyz));
    float3 b = cross(normal, t) * tangent.w;
    return float3(dot(v, t), dot(v, b), dot(v, normal));
}

float3 rikka_graph_from_tangent(float3 v, float3 normal, float4 tangent) {
    float3 t = normalize(tangent.xyz - normal * dot(normal, tangent.xyz));
    float3 b = cross(normal, t) * tangent.w;
    return t * v.x + b * v.y + normal * v.z;
}

float2 rikka_graph_parallax(float2 uv, float height, float scale, float3 view) {
    float lifted = height * scale - scale * 0.5;
    return uv + lifted * view.xy / (view.z + 0.42);
}

float3 rikka_graph_normal_map(float3 normal, float4 tangent, float3 tangent_normal, float strength) {
    float3 scaled = float3(tangent_normal.xy * strength, tangent_normal.z);
    return normalize(rikka_graph_from_tangent(normalize(scaled), normal, tangent));
}

float3 rikka_graph_rgb_to_hsv(float3 c) {
    float4 k = float4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    float4 p = c.g < c.b ? float4(c.bg, k.wz) : float4(c.gb, k.xy);
    float4 q = c.r < p.x ? float4(p.xyw, c.r) : float4(c.r, p.yzx);
    float d = q.x - min(q.w, q.y);
    return float3(abs(q.z + (q.w - q.y) / (6.0 * d + 1e-10)), d / (q.x + 1e-10), q.x);
}

float3 rikka_graph_hsv_to_rgb(float3 c) {
    float3 p = abs(frac(c.xxx + float3(1.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0);
    return c.z * lerp(float3(1.0), saturate(p - 1.0), c.y);
}

float3 rikka_graph_saturation(float3 c, float amount) {
    return lerp(float3(rikka_luma(c)), c, amount);
}

float3 rikka_graph_blend_overlay(float3 base, float3 blend) {
    return lerp(2.0 * base * blend, 1.0 - 2.0 * (1.0 - base) * (1.0 - blend), step(0.5, base));
}

float3 rikka_graph_blend_soft_light(float3 base, float3 blend) {
    float3 dark = base - (1.0 - 2.0 * blend) * base * (1.0 - base);
    float3 light = base + (2.0 * blend - 1.0) * (sqrt(base) - base);
    return lerp(dark, light, step(0.5, blend));
}

float3 rikka_graph_blend_dodge(float3 base, float3 blend) {
    return min(base / max(1.0 - blend, 1e-5), 1.0);
}

float3 rikka_graph_blend_burn(float3 base, float3 blend) {
    return 1.0 - min((1.0 - base) / max(blend, 1e-5), 1.0);
}

float3 rikka_graph_blend_divide(float3 base, float3 blend) {
    return float3(
        blend.x != 0.0 ? base.x / blend.x : 0.0,
        blend.y != 0.0 ? base.y / blend.y : 0.0,
        blend.z != 0.0 ? base.z / blend.z : 0.0);
}

float3 rikka_graph_blend_hue(float3 base, float3 blend) {
    float3 top = rikka_graph_rgb_to_hsv(blend);
    if (top.y == 0.0) {
        return base;
    }
    float3 hsv = rikka_graph_rgb_to_hsv(base);
    return rikka_graph_hsv_to_rgb(float3(top.x, hsv.y, hsv.z));
}

float3 rikka_graph_blend_saturation(float3 base, float3 blend) {
    float3 hsv = rikka_graph_rgb_to_hsv(base);
    if (hsv.y == 0.0) {
        return base;
    }
    return rikka_graph_hsv_to_rgb(float3(hsv.x, rikka_graph_rgb_to_hsv(blend).y, hsv.z));
}

float3 rikka_graph_blend_color(float3 base, float3 blend) {
    float3 top = rikka_graph_rgb_to_hsv(blend);
    if (top.y == 0.0) {
        return base;
    }
    return rikka_graph_hsv_to_rgb(float3(top.x, top.y, rikka_graph_rgb_to_hsv(base).z));
}

float3 rikka_graph_blend_value(float3 base, float3 blend) {
    float3 hsv = rikka_graph_rgb_to_hsv(base);
    return rikka_graph_hsv_to_rgb(float3(hsv.x, hsv.y, rikka_graph_rgb_to_hsv(blend).z));
}

float rikka_graph_smooth(float low, float high, float value) {
    float t = clamp((value - low) / (high - low), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

float rikka_graph_smoother(float low, float high, float value) {
    float t = clamp((value - low) / (high - low), 0.0, 1.0);
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
}

float rikka_graph_map_range(
    float value, float from_low, float from_high, float to_low, float to_high, float steps, float clamped, float mode)
{
    int kind = int(mode);
    float t = 0.0;
    if (from_low != from_high) {
        if (kind == 2) {
            t = from_low > from_high
                ? 1.0 - rikka_graph_smooth(from_high, from_low, value)
                : rikka_graph_smooth(from_low, from_high, value);
        } else if (kind == 3) {
            t = from_low > from_high
                ? 1.0 - rikka_graph_smoother(from_high, from_low, value)
                : rikka_graph_smoother(from_low, from_high, value);
        } else {
            t = (value - from_low) / (from_high - from_low);
            if (kind == 1) {
                t = steps > 0.0 ? floor(t * (steps + 1.0)) / steps : 0.0;
            }
        }
    }
    float result = to_low + t * (to_high - to_low);
    if (clamped > 0.5) {
        result = clamp(result, min(to_low, to_high), max(to_low, to_high));
    }
    return result;
}

float rikka_graph_compare(float a, float b, float epsilon, float mode) {
    switch (int(mode)) {
    case 0:
        return a < b ? 1.0 : 0.0;
    case 1:
        return a <= b ? 1.0 : 0.0;
    case 2:
        return a > b ? 1.0 : 0.0;
    case 3:
        return a >= b ? 1.0 : 0.0;
    case 4:
        return abs(a - b) <= epsilon ? 1.0 : 0.0;
    default:
        return abs(a - b) > epsilon ? 1.0 : 0.0;
    }
}

float4 rikka_graph_white_noise(float3 p, float seed) {
    uint3 bits = asuint(p) ^ (asuint(seed) * uint3(1u, 0x9e3779b9u, 0x85ebca6bu));
    float3 h = float3(rikka_graph_pcg3(bits)) * (1.0 / 4294967295.0);
    return float4(h, h.x);
}

float4 rikka_graph_checker(float3 p, float3 first, float3 second) {
    int3 cell = int3(floor(p));
    float fac = ((cell.x + cell.y + cell.z) & 1) == 0 ? 1.0 : 0.0;
    return float4(lerp(second, first, fac), fac);
}

struct RikkaGraphBrick {
    float mortar_size;
    float mortar_smooth;
    float bias;
    float width;
    float height;
    float offset;
    float offset_frequency;
    float squash;
    float squash_frequency;
};

RikkaGraphBrick rikka_graph_brick_shape(
    float mortar_size, float mortar_smooth, float bias, float width, float height,
    float offset, float offset_frequency, float squash, float squash_frequency)
{
    RikkaGraphBrick b;
    b.mortar_size = mortar_size;
    b.mortar_smooth = mortar_smooth;
    b.bias = bias;
    b.width = width;
    b.height = height;
    b.offset = offset;
    b.offset_frequency = offset_frequency;
    b.squash = squash;
    b.squash_frequency = squash_frequency;
    return b;
}

float4 rikka_graph_brick(float3 p, float3 first, float3 second, float3 mortar_color, RikkaGraphBrick b) {
    float height = max(b.height, 1e-5);
    float row = floor(p.y / height);
    float width = max(b.width, 1e-5);
    int offset_frequency = max(int(b.offset_frequency), 1);
    int squash_frequency = max(int(b.squash_frequency), 1);
    int rows = int(row);
    if (rows % squash_frequency == 0) {
        width *= max(b.squash, 1e-5);
    }
    float shift = rows % offset_frequency == 0 ? width * b.offset : 0.0;
    float column = floor((p.x + shift) / width);
    float x = p.x + shift - width * column;
    float y = p.y - height * row;
    float tint = clamp(rikka_graph_hash3(float3(row, column, 0.0)).x + b.bias, 0.0, 1.0);
    float edge = min(min(x, y), min(width - x, height - y));
    float mortar = 0.0;
    if (edge < b.mortar_size) {
        mortar = b.mortar_smooth <= 0.0
            ? 1.0
            : rikka_graph_smooth(0.0, b.mortar_smooth, 1.0 - edge / max(b.mortar_size, 1e-5));
    }
    return float4(lerp(lerp(first, second, tint), mortar_color, mortar), mortar);
}

float rikka_graph_wave(
    float3 p, float distortion, float detail, float detail_scale, float roughness, float phase,
    float kind, float direction, float profile)
{
    int axis = int(direction);
    float n;
    if (int(kind) == 0) {
        n = axis == 0 ? p.x * 20.0 : axis == 1 ? p.y * 20.0 : axis == 2 ? p.z * 20.0 : (p.x + p.y + p.z) * 10.0;
    } else {
        float3 ring = axis == 0 ? float3(0.0, p.y, p.z) : axis == 1 ? float3(p.x, 0.0, p.z) : axis == 2 ? float3(p.x, p.y, 0.0) : p;
        n = length(ring) * 20.0;
    }
    n += phase;
    if (distortion != 0.0) {
        n += distortion * (rikka_graph_fbm_gradient(p * detail_scale, detail + 1.0, 2.0, roughness) * 2.0 - 1.0);
    }
    int shape = int(profile);
    if (shape == 1) {
        n /= RIKKA_TAU;
        return n - floor(n);
    }
    if (shape == 2) {
        n /= RIKKA_TAU;
        return abs(n - floor(n + 0.5)) * 2.0;
    }
    return 0.5 + 0.5 * sin(n - RIKKA_PI * 0.5);
}

float rikka_graph_gradient(float3 p, float kind) {
    switch (int(kind)) {
    case 1: {
        float r = max(p.x, 0.0);
        return r * r;
    }
    case 2: {
        float r = clamp(p.x, 0.0, 1.0);
        return r * r * (3.0 - 2.0 * r);
    }
    case 3:
        return (p.x + p.y) * 0.5;
    case 4:
        return max(1.0 - length(p), 0.0);
    case 5: {
        float r = max(1.0 - length(p), 0.0);
        return r * r;
    }
    case 6:
        return atan2(p.y, p.x) / RIKKA_TAU + 0.5;
    default:
        return p.x;
    }
}

float3x3 rikka_graph_euler(float3 angles) {
    float3 c = cos(angles);
    float3 s = sin(angles);
    float3 x = float3(c.y * c.z, c.y * s.z, -s.y);
    float3 y = float3(s.x * s.y * c.z - c.x * s.z, s.x * s.y * s.z + c.x * c.z, s.x * c.y);
    float3 z = float3(c.x * s.y * c.z + s.x * s.z, c.x * s.y * s.z - s.x * c.z, c.x * c.y);
    return rikka_columns(x, y, z);
}

float3 rikka_graph_mapping(float3 v, float3 location, float3 rotation, float3 scale, float kind) {
    float3x3 turn = rikka_graph_euler(rotation);
    switch (int(kind)) {
    case 1: {
        float3 safe = float3(
            scale.x != 0.0 ? 1.0 / scale.x : 0.0,
            scale.y != 0.0 ? 1.0 / scale.y : 0.0,
            scale.z != 0.0 ? 1.0 / scale.z : 0.0);
        return mul(transpose(turn), v - location) * safe;
    }
    case 2:
        return mul(turn, v * scale);
    case 3: {
        float3 safe = float3(
            scale.x != 0.0 ? v.x / scale.x : 0.0,
            scale.y != 0.0 ? v.y / scale.y : 0.0,
            scale.z != 0.0 ? v.z / scale.z : 0.0);
        float3 turned = mul(turn, safe);
        float length_of = length(turned);
        return length_of > 0.0 ? turned / length_of : turned;
    }
    default:
        return mul(turn, v * scale) + location;
    }
}

float3 rikka_graph_axis_angle(float3 v, float3 axis, float angle) {
    float length_of = length(axis);
    if (length_of <= 0.0) {
        return v;
    }
    float3 k = axis / length_of;
    float c = cos(angle);
    float s = sin(angle);
    return v * c + cross(k, v) * s + k * dot(k, v) * (1.0 - c);
}

float3 rikka_graph_vector_rotate(
    float3 v, float3 center, float3 axis, float angle, float3 rotation, float kind, float inverted)
{
    float sign_of = inverted > 0.5 ? -1.0 : 1.0;
    float3 d = v - center;
    switch (int(kind)) {
    case 1:
        return rikka_graph_axis_angle(d, float3(1.0, 0.0, 0.0), angle * sign_of) + center;
    case 2:
        return rikka_graph_axis_angle(d, float3(0.0, 1.0, 0.0), angle * sign_of) + center;
    case 3:
        return rikka_graph_axis_angle(d, float3(0.0, 0.0, 1.0), angle * sign_of) + center;
    case 4: {
        float3x3 turn = rikka_graph_euler(rotation);
        return (inverted > 0.5 ? mul(transpose(turn), d) : mul(turn, d)) + center;
    }
    default:
        return rikka_graph_axis_angle(d, axis, angle * sign_of) + center;
    }
}

float3 rikka_graph_hue_saturation(float3 color, float hue, float saturation, float value, float factor) {
    float3 hsv = rikka_graph_rgb_to_hsv(color);
    hsv.x = frac(hsv.x + hue + 0.5);
    hsv.y = clamp(hsv.y * saturation, 0.0, 1.0);
    hsv.z = hsv.z * value;
    return lerp(color, rikka_graph_hsv_to_rgb(hsv), factor);
}

float3 rikka_graph_brightness_contrast(float3 color, float brightness, float contrast) {
    float a = 1.0 + contrast;
    float b = brightness - contrast * 0.5;
    return max(a * color + b, float3(0.0));
}

float3 rikka_graph_gamma(float3 color, float gamma) {
    return float3(
        color.x > 0.0 ? pow(color.x, gamma) : color.x,
        color.y > 0.0 ? pow(color.y, gamma) : color.y,
        color.z > 0.0 ? pow(color.z, gamma) : color.z);
}

float3 rikka_graph_xyz_to_rgb(float3 xyz) {
    return float3(
        3.2404542 * xyz.x - 1.5371385 * xyz.y - 0.4985314 * xyz.z,
        -0.9692660 * xyz.x + 1.8760108 * xyz.y + 0.0415560 * xyz.z,
        0.0556434 * xyz.x - 0.2040259 * xyz.y + 1.0572252 * xyz.z);
}

float3 rikka_graph_blackbody(float kelvin) {
    float t = clamp(kelvin, 1667.0, 25000.0);
    float t1 = 1000.0 / t;
    float x = t <= 4000.0
        ? ((-0.2661239 * t1 - 0.2343589) * t1 + 0.8776956) * t1 + 0.179910
        : ((-3.0258469 * t1 + 2.1070379) * t1 + 0.2226347) * t1 + 0.240390;
    float y = t <= 2222.0
        ? ((-1.1063814 * x - 1.34811020) * x + 2.18555832) * x - 0.20219683
        : t <= 4000.0
            ? ((-0.9549476 * x - 1.37418593) * x + 2.09137015) * x - 0.16748867
            : ((3.0817580 * x - 5.87338670) * x + 3.75112997) * x - 0.37001483;
    return max(rikka_graph_xyz_to_rgb(float3(x / y, 1.0, (1.0 - x - y) / y)), float3(0.0));
}

float rikka_graph_lobe(float wavelength, float center, float below, float above) {
    float t = (wavelength - center) / (wavelength < center ? below : above);
    return exp(-0.5 * t * t);
}

float3 rikka_graph_wavelength(float nanometres) {
    float l = nanometres;
    float x = 1.056 * rikka_graph_lobe(l, 599.8, 37.9, 31.0) + 0.362 * rikka_graph_lobe(l, 442.0, 16.0, 26.7)
        - 0.065 * rikka_graph_lobe(l, 501.1, 20.4, 26.2);
    float y = 0.821 * rikka_graph_lobe(l, 568.8, 46.9, 40.5) + 0.286 * rikka_graph_lobe(l, 530.9, 16.3, 31.1);
    float z = 1.217 * rikka_graph_lobe(l, 437.0, 11.8, 36.0) + 0.681 * rikka_graph_lobe(l, 459.0, 26.0, 13.8);
    return max(rikka_graph_xyz_to_rgb(float3(x, y, z)), float3(0.0));
}

float3 rikka_graph_luminance_add(float3 base, float3 added) {
    float3 hsv = rikka_graph_rgb_to_hsv(base + added);
    hsv.z = max(max(max(base.x, base.y), base.z), max(max(added.x, added.y), added.z));
    return rikka_graph_hsv_to_rgb(hsv);
}

float3 rikka_graph_saturated_shade(float3 color, float amount) {
    float mean = (color.x + color.y + color.z) / 3.0;
    float3 kept = 0.4372 * color + 0.5628 * saturate(color / max(mean, 1e-4));
    return lerp(color, kept, amount);
}

float3 rikka_graph_tone_curve(float3 color, float curve, float lift) {
    float3 c = max(color, float3(0.0));
    c += max(c - 0.6, float3(0.0)) * lift;
    switch (int(curve)) {
    case 1:
        return saturate((1.36 * c + 0.047) * c / ((0.93 * c + 0.56) * c + 0.14));
    case 2:
        return saturate(c * (2.8 * c + 0.4) / (c * (2.1 * c + 0.5) + 1.5));
    case 3:
        return c / (1.0 + c);
    default:
        return saturate(c * (2.51 * c + 0.03) / (c * (2.43 * c + 0.59) + 0.14));
    }
}

float3 rikka_graph_bump(float3 position, float3 normal, float height, float strength, float distance, float inverted) {
    float3 dx = ddx(position);
    float3 dy = ddy(position);
    float3 rx = cross(dy, normal);
    float3 ry = cross(normal, dx);
    float determinant = dot(dx, rx);
    float scaled = height * distance * (inverted > 0.5 ? -1.0 : 1.0);
    float3 gradient = (determinant < 0.0 ? -1.0 : 1.0) * (ddx(scaled) * rx + ddy(scaled) * ry);
    float3 bumped = normalize(abs(determinant) * normal - gradient);
    return normalize(lerp(normal, bumped, clamp(strength, 0.0, 1.0)));
}

float rikka_graph_min_component(float v) {
    return v;
}

float rikka_graph_min_component(float2 v) {
    return min(v.x, v.y);
}

float rikka_graph_min_component(float3 v) {
    return min(min(v.x, v.y), v.z);
}

float rikka_graph_min_component(float4 v) {
    return min(min(v.x, v.y), min(v.z, v.w));
}

float rikka_graph_max_component(float v) {
    return v;
}

float rikka_graph_max_component(float2 v) {
    return max(v.x, v.y);
}

float rikka_graph_max_component(float3 v) {
    return max(max(v.x, v.y), v.z);
}

float rikka_graph_max_component(float4 v) {
    return max(max(v.x, v.y), max(v.z, v.w));
}
