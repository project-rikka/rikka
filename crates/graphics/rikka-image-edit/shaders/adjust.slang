import plane;
import color;
import floating;

struct Push {
    Head head;
    float* params;
    float* tables;
    uint count;
    uint transfer;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint CURVES = 0u;
static const uint EXPOSURE = 1u;
static const uint HUE_SATURATION = 2u;
static const uint COLOR_BALANCE = 3u;
static const uint BLACK_WHITE = 4u;
static const uint PHOTO_FILTER = 5u;
static const uint CHANNEL_MIXER = 6u;
static const uint INVERT = 7u;
static const uint THRESHOLD = 8u;
static const uint GRADIENT_MAP = 9u;
static const uint SELECTIVE_COLOR = 10u;
static const uint VIBRANCE = 11u;
static const uint LOOKUP = 12u;
static const uint DESATURATE = 13u;
static const uint WHITE_BALANCE = 14u;

float p(uint index) {
    return pc.params[index];
}

float curve(uint table, float v) {
    uint n = pc.count;
    float position = clamp(v, 0.0, 1.0) * float(n - 1u);
    uint low = min(uint(position), n - 2u);
    float t = position - float(low);
    uint at = table * n + low;
    return lerp(p(at), p(at + 1u), t);
}

float3 curves(float3 c) {
    return float3(curve(0u, curve(1u, c.r)), curve(0u, curve(2u, c.g)), curve(0u, curve(3u, c.b)));
}

float hue_mask(float hue, uint at) {
    float a = p(at);
    float wrap_left = floored_mod(p(at + 1u) - a, 360.0);
    float wrap_center = floored_mod(p(at + 2u) - a, 360.0);
    float wrap_right = floored_mod(p(at + 3u) - a, 360.0);
    float centered = floored_mod(hue - a, 360.0);
    if (centered >= wrap_left && centered <= wrap_center) {
        return 1.0;
    }
    if (centered < wrap_left) {
        return wrap_left > 0.0 ? centered / wrap_left : 0.0;
    }
    if (centered <= wrap_right) {
        return wrap_right > wrap_center ? (wrap_right - centered) / (wrap_right - wrap_center) : 0.0;
    }
    return 0.0;
}

float lightness(float v, float amount) {
    return amount > 0.0 ? v * (1.0 - amount) + amount : v * (1.0 + amount);
}

float saturated(float v, float amount) {
    return amount > 0.0 ? clamp(v / (1.0 - amount + 0.01), 0.0, 1.0) : clamp(v * (1.0 + amount), 0.0, 1.0);
}

float3 hue_saturation(float3 c) {
    if (p(0u) != 0.0) {
        float3 base = rgb_to_hsl(c);
        return hsl_to_rgb(float3(p(1u), p(2u), lightness(base.z, p(3u))));
    }
    float3 light = float3(lightness(c.r, p(6u)), lightness(c.g, p(6u)), lightness(c.b, p(6u)));
    float3 hsl = rgb_to_hsl(light);
    float hue_shift = 0.0;
    float saturation_shift = 0.0;
    float lightness_shift = 0.0;
    for (uint range = 0u; range < 6u; range++) {
        uint at = 7u + range * 7u;
        float weight = hsl.y > 0.0 ? hue_mask(hsl.x, at) : 0.0;
        if (weight <= 0.0) {
            continue;
        }
        hue_shift += p(at + 4u) * weight;
        saturation_shift += p(at + 5u) * weight;
        lightness_shift += p(at + 6u) * weight;
    }
    float hue = floored_mod(hsl.x + hue_shift + p(4u), 360.0);
    float s = saturated(saturated(hsl.y, p(5u)), clamp(saturation_shift, -1.0, 1.0));
    float3 out_rgb = hsl_to_rgb(float3(hue, s, hsl.z));
    if (lightness_shift != 0.0) {
        float amount = clamp(lightness_shift, -1.0, 1.0) * s;
        out_rgb = float3(lightness(out_rgb.r, amount), lightness(out_rgb.g, amount), lightness(out_rgb.b, amount));
    }
    return out_rgb;
}

float3 color_balance(float3 c) {
    const float a = 0.25;
    const float b = 0.333;
    const float scale = 0.7;
    float3 result;
    for (int channel = 0; channel < 3; channel++) {
        float v = c[channel];
        float shadow = clamp((v - b) / -a + 0.5, 0.0, 1.0) * scale;
        float mid = clamp((v - b) / a + 0.5, 0.0, 1.0) * clamp((v + b - 1.0) / -a + 0.5, 0.0, 1.0) * scale;
        float high = clamp((v + b - 1.0) / a + 0.5, 0.0, 1.0) * scale;
        result[channel] = clamp(
            v + p(uint(channel)) * shadow + p(3u + uint(channel)) * mid + p(6u + uint(channel)) * high,
            0.0, 1.0);
    }
    if (p(9u) != 0.0) {
        float3 original = rgb_to_hsl(c);
        float3 changed = rgb_to_hsl(result);
        result = hsl_to_rgb(float3(changed.x, changed.y, original.z));
    }
    return result;
}

float3 black_white(float3 c) {
    float high = max(c.r, max(c.g, c.b));
    float low = min(c.r, min(c.g, c.b));
    float mid = c.r + c.g + c.b - high - low;
    uint top = c.r >= c.g && c.r >= c.b ? 0u : (c.g >= c.b ? 1u : 2u);
    uint bottom = c.r < c.g && c.r < c.b ? 0u : (c.g < c.b ? 1u : 2u);
    if (top == bottom) {
        bottom = (top + 1u) % 3u;
    }
    uint middle = 3u - top - bottom;
    float primary = top == 0u ? p(0u) : (top == 1u ? p(2u) : p(4u));
    float secondary;
    if ((top == 0u && middle == 1u) || (top == 1u && middle == 0u)) {
        secondary = p(1u);
    } else if ((top == 1u && middle == 2u) || (top == 2u && middle == 1u)) {
        secondary = p(3u);
    } else {
        secondary = p(5u);
    }
    float gray = clamp(low + (mid - low) * secondary + (high - mid) * primary, 0.0, 1.0);
    if (p(6u) != 0.0) {
        return set_lum(float3(p(7u), p(8u), p(9u)), gray);
    }
    return float3(gray);
}

float3 photo_filter(float3 c) {
    float3 tint = float3(p(0u), p(1u), p(2u));
    float3 filtered = c + (c * tint - c) * p(3u);
    return p(4u) != 0.0 ? set_lum(filtered, lum(c)) : filtered;
}

float3 channel_mixer(float3 c) {
    if (p(0u) != 0.0) {
        float gray = dot(c, float3(p(13u), p(14u), p(15u))) + p(16u);
        return float3(clamp(gray, 0.0, 1.0));
    }
    float3 result;
    for (uint row = 0u; row < 3u; row++) {
        uint at = 1u + row * 4u;
        result[row] = clamp(dot(c, float3(p(at), p(at + 1u), p(at + 2u))) + p(at + 3u), 0.0, 1.0);
    }
    return result;
}

float3 gradient_map(float3 c) {
    float t = clamp(lum(c), 0.0, 1.0);
    if (p(0u) != 0.0) {
        t = 1.0 - t;
    }
    uint n = pc.count;
    float position = t * float(n - 1u);
    uint low = min(uint(position), n - 2u);
    float f = position - float(low);
    uint a = 1u + low * 3u;
    uint b = a + 3u;
    return lerp(float3(p(a), p(a + 1u), p(a + 2u)), float3(p(b), p(b + 1u), p(b + 2u)), f);
}

float3 selective_color(float3 c) {
    float high = max(c.r, max(c.g, c.b));
    float low = min(c.r, min(c.g, c.b));
    float mid = c.r + c.g + c.b - high - low;
    float weights[10];
    for (int i = 0; i < 10; i++) {
        weights[i] = 0.0;
    }
    if (c.r == high) weights[1] = high - mid;
    if (c.b == low) weights[2] = mid - low;
    if (c.g == high) weights[3] = high - mid;
    if (c.r == low) weights[4] = mid - low;
    if (c.b == high) weights[5] = high - mid;
    if (c.g == low) weights[6] = mid - low;
    weights[7] = max((low - 0.5) * 2.0, 0.0);
    weights[9] = max((0.5 - high) * 2.0, 0.0);
    weights[8] = max(1.0 - abs(high - 0.5) - abs(low - 0.5), 0.0);
    bool absolute = p(0u) != 0.0;
    float3 ink = 1.0 - c;
    float black = 1.0 - high;
    for (uint plate = 1u; plate < 10u; plate++) {
        float weight = weights[plate];
        if (weight <= 0.0) {
            continue;
        }
        uint at = 1u + plate * 4u;
        for (int channel = 0; channel < 3; channel++) {
            float base = absolute ? 1.0 : ink[channel];
            float black_base = absolute ? 1.0 : black;
            ink[channel] += weight * (p(at + uint(channel)) * base + p(at + 3u) * black_base);
        }
    }
    return clamp(1.0 - ink, 0.0, 1.0);
}

float3 vibrance(float3 c) {
    float3 hsl = rgb_to_hsl(c);
    float weight = (1.0 - hsl.y) * (1.0 - hsl.y);
    float adjusted = saturated(saturated(hsl.y, p(1u)), p(0u) * weight);
    return hsl_to_rgb(float3(hsl.x, adjusted, hsl.z));
}

float3 lookup_at(uint n, uint3 at) {
    uint index = 7u + 3u * ((at.b * n + at.g) * n + at.r);
    return float3(p(index), p(index + 1u), p(index + 2u));
}

float3 lookup(float3 c) {
    uint n = uint(p(0u));
    float3 low_domain = float3(p(1u), p(2u), p(3u));
    float3 high_domain = float3(p(4u), p(5u), p(6u));
    float last = float(n - 1u);
    float3 position = clamp((c - low_domain) / max(high_domain - low_domain, float3(1e-6)), 0.0, 1.0) * last;
    uint3 low = uint3(min(floor(position), float3(last)));
    uint3 high = min(low + 1u, uint3(n - 1u));
    float3 t = position - float3(low);
    float3 x00 = lerp(lookup_at(n, uint3(low.r, low.g, low.b)), lookup_at(n, uint3(high.r, low.g, low.b)), t.r);
    float3 x10 = lerp(lookup_at(n, uint3(low.r, high.g, low.b)), lookup_at(n, uint3(high.r, high.g, low.b)), t.r);
    float3 x01 = lerp(lookup_at(n, uint3(low.r, low.g, high.b)), lookup_at(n, uint3(high.r, low.g, high.b)), t.r);
    float3 x11 = lerp(lookup_at(n, uint3(low.r, high.g, high.b)), lookup_at(n, uint3(high.r, high.g, high.b)), t.r);
    float3 looked = lerp(lerp(x00, x10, t.g), lerp(x01, x11, t.g), t.b);
    return clamp(looked, 0.0, 1.0);
}

float3 desaturate(float3 c) {
    uint method = uint(p(0u));
    float high = max(c.r, max(c.g, c.b));
    float low = min(c.r, min(c.g, c.b));
    if (method == 0u) {
        return float3((high + low) * 0.5);
    }
    if (method == 1u) {
        return float3(lum(c));
    }
    return float3((c.r + c.g + c.b) / 3.0);
}

float3 white_balance(float3 c) {
    float3 linear = to_linear(c, pc.transfer, pc.tables);
    float3 balanced;
    for (uint row = 0u; row < 3u; row++) {
        balanced[row] = dot(float3(p(row * 3u), p(row * 3u + 1u), p(row * 3u + 2u)), linear);
    }
    return from_linear(max(balanced, 0.0), pc.transfer, pc.tables);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void adjust(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 c = pc.head.src.load(pixel);
    float3 rgb = c.rgb;
    switch (pc.head.mode) {
    case CURVES: rgb = curves(rgb); break;
    case EXPOSURE: {
        float3 linear = pow(max(rgb, 0.0), float3(2.2)) * exp2(p(0u)) + p(1u);
        rgb = pow(pow(clamp(linear, 0.0, 1.0), float3(1.0 / p(2u))), float3(float(1.0l / 2.2l)));
        break;
    }
    case HUE_SATURATION: rgb = hue_saturation(clamp(rgb, 0.0, 1.0)); break;
    case COLOR_BALANCE: rgb = color_balance(clamp(rgb, 0.0, 1.0)); break;
    case BLACK_WHITE: rgb = black_white(clamp(rgb, 0.0, 1.0)); break;
    case PHOTO_FILTER: rgb = photo_filter(rgb); break;
    case CHANNEL_MIXER: rgb = channel_mixer(rgb); break;
    case INVERT: rgb = 1.0 - rgb; break;
    case THRESHOLD: rgb = float3(lum(rgb) >= p(0u) ? 1.0 : 0.0); break;
    case GRADIENT_MAP: rgb = gradient_map(rgb); break;
    case SELECTIVE_COLOR: rgb = selective_color(clamp(rgb, 0.0, 1.0)); break;
    case VIBRANCE: rgb = vibrance(clamp(rgb, 0.0, 1.0)); break;
    case LOOKUP: rgb = lookup(rgb); break;
    case DESATURATE: rgb = desaturate(rgb); break;
    case WHITE_BALANCE: rgb = white_balance(rgb); break;
    }
    pc.head.dst.store(pixel, pc.head.clamped(float4(rgb, c.a)));
}
