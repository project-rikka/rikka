__exported import rikka.npr.head;

float2 rikka_npr_uv_transform(
    float2 uv, float2 offset, float2 scale, float rotation, float2 scroll, float spin, float mask, float time)
{
    float turn = spin * time * mask;
    float2 animated = uv;
    if (turn != 0.0) {
        float c = cos(turn);
        float s = sin(turn);
        animated = mul(float2x2(c, -s, s, c), uv - 0.5) + 0.5;
    }
    animated += scroll * time * mask;
    return rikka_uv_transform(animated, offset, scale, rotation);
}

float rikka_npr_wrap(float3 normal, float3 light, float wrap) {
    return (dot(normal, light) + wrap) / (1.0 + wrap);
}

struct RikkaNprThreshold {
    float value;
    float force_lit;
    float force_shadow;
};

RikkaNprThreshold rikka_npr_threshold(
    float facing, float bias, float mode, float occlusion, float shift, float scale, float lit_above, float shadow_below)
{
    RikkaNprThreshold t;
    t.force_lit = 0.0;
    t.force_shadow = 0.0;
    int kind = int(mode);
    if (kind == 2) {
        t.value = facing + shift + bias * occlusion * scale;
        return t;
    }
    float level;
    if (kind == 1) {
        level = bias * occlusion;
        t.value = min(1.0, rikka_npr_half(facing) * 2.0 * level);
    } else {
        float x = bias - 0.5;
        level = (2.0 * x * abs(x) + 0.5) * occlusion;
        t.value = (rikka_npr_half(facing) + level) * 0.5;
    }
    t.force_lit = level > lit_above ? 1.0 : 0.0;
    t.force_shadow = level < shadow_below ? 1.0 : 0.0;
    t.value = t.force_lit > 0.5 ? 1.0 : t.force_shadow > 0.5 ? 0.0 : t.value;
    return t;
}

float rikka_npr_toon_step(float value, float edge, float feather, float mode, float toony, float softness) {
    float half_width = max(feather, 0.0) * 0.5;
    switch (int(mode)) {
    case 1:
        return half_width <= 1e-5 ? step(edge, value) : rikka_npr_linearstep(edge - half_width, edge + half_width, value);
    case 2: {
        float reach = max(1.0 - toony, 1e-5);
        return rikka_npr_linearstep(edge - reach, edge + reach, value);
    }
    case 3:
        return value >= edge ? 1.0 : 1.0 - pow(saturate((edge - value) / max(feather, 1e-5)), max(softness, 1e-3));
    default:
        return half_width <= 1e-5 ? step(edge, value) : smoothstep(edge - half_width, edge + half_width, value);
    }
}

struct RikkaNprBands {
    float lit;
    float shallow;
    float deep;
    float upper_edge;
    float lower_edge;
    float3 color;
};

RikkaNprBands rikka_npr_bands(
    float value, float low, float high, float smoothness, float3 lit_color, float3 shallow_color, float3 deep_color)
{
    float k = max(smoothness, 1e-5) * 0.5;
    float upper = rikka_npr_linearstep(high - k, high + k, value);
    float lower = rikka_npr_linearstep(low - k, low + k, value);
    RikkaNprBands b;
    b.lit = upper;
    b.shallow = max(lower - upper, 0.0);
    b.deep = 1.0 - max(lower, upper);
    b.upper_edge = 4.0 * upper * (1.0 - upper);
    b.lower_edge = 4.0 * lower * (1.0 - lower);
    b.color = b.lit * lit_color + b.shallow * shallow_color + b.deep * deep_color;
    return b;
}

float4 rikka_npr_ramp_row(RikkaTexture ramp, float u, float row, float rows, float origin) {
    float v = (clamp(round(row), 0.0, max(rows, 1.0) - 1.0) + 0.5) / max(rows, 1.0);
    v = int(origin) == 1 ? 1.0 - v : v;
    return ramp.sampled_level(float2(clamp(u, 0.002, 0.998), v), 0.0);
}

float4 rikka_npr_ramp(
    RikkaTexture ramp, float u, float row, float rows, float group, float row_in_group, float offset, float origin,
    float u_bias, float second_offset, float blend)
{
    float at = round(row) * max(round(group), 1.0) + round(row_in_group) + round(offset);
    float shifted = u + u_bias;
    float4 first = rikka_npr_ramp_row(ramp, shifted, at, rows, origin);
    if (blend <= 0.0) {
        return first;
    }
    return lerp(first, rikka_npr_ramp_row(ramp, shifted, at + round(second_offset), rows, origin), saturate(blend));
}

struct RikkaNprVerticalRamp {
    float3 color;
    float3 top;
};

RikkaNprVerticalRamp rikka_npr_vertical_ramp(RikkaTexture ramp, float facing) {
    RikkaNprVerticalRamp r;
    r.color = ramp.sampled_level(float2(0.5, 0.5 - 0.5 * clamp(facing, -1.0, 1.0)), 0.0).rgb;
    r.top = ramp.sampled_level(float2(0.5, 0.0), 0.0).rgb;
    return r;
}

float rikka_npr_material_id(float value, float mode, float count) {
    float n = max(round(count), 1.0);
    switch (int(mode)) {
    case 1:
        return clamp(floor(n * value), 0.0, n - 1.0);
    case 2:
        return clamp(n - 1.0 - floor(n * value), 0.0, n - 1.0);
    case 3:
        return clamp(round(value * (n - 1.0)), 0.0, n - 1.0);
    default:
        return value >= 0.8 ? 1.0 : value >= 0.6 ? 4.0 : value >= 0.4 ? 2.0 : value >= 0.2 ? 3.0 : 0.0;
    }
}

float4 rikka_npr_param_lut(RikkaTexture table, float column, float row, float columns, float rows) {
    float u = (clamp(round(column), 0.0, max(columns, 1.0) - 1.0) + 0.5) / max(columns, 1.0);
    float v = (clamp(round(row), 0.0, max(rows, 1.0) - 1.0) + 0.5) / max(rows, 1.0);
    return table.sampled_level(float2(u, v), 0.0);
}

float3 rikka_npr_shade_mix(float3 albedo, float lit, float3 shade, float mode) {
    float t = saturate(lit);
    switch (int(mode)) {
    case 1:
        return albedo * shade;
    case 2:
        return lerp(shade, albedo, t);
    default:
        return albedo * lerp(shade, float3(1.0), t);
    }
}

float3 rikka_npr_skin_levels(
    float3 ramp, float shadow_area, float skin, float low, float middle, float high,
    float3 skin_light, float3 skin_shadow, float3 other_light, float3 other_shadow)
{
    bool is_skin = skin > 0.5;
    float3 light = saturate((ramp - middle) / max(high - middle, 1e-5) * 0.5 + 0.5) * (is_skin ? skin_light : other_light);
    float3 shadow = saturate(-(middle - ramp) / max(middle - low, 1e-5) * 0.5 + 0.5) * (is_skin ? skin_shadow : other_shadow);
    return shadow_area < 0.9 ? shadow : light;
}

float rikka_npr_brush(float value, RikkaTexture brush, float2 uv, float scale, float amount, float rotation) {
    float c = cos(rotation);
    float s = sin(rotation);
    float2 at = uv * scale;
    at = float2(c * at.x - s * at.y, s * at.x + c * at.y);
    return value + (brush.sampled_level(at, 0.0).r - 0.5) * amount;
}

struct RikkaNprEmission {
    float3 emission;
    float3 boost;
};

RikkaNprEmission rikka_npr_emission(
    float3 albedo, float mask, float threshold, float3 color, float strength, float3 bloom_color, float bloom_strength)
{
    RikkaNprEmission e;
    e.emission = mask > threshold ? albedo * color * strength : float3(0.0);
    e.boost = float3(1.0) + bloom_strength * bloom_color;
    return e;
}

float3 rikka_npr_expression(float3 color, float3 mask, float3 red, float3 green, float3 blue, float3 strength) {
    float3 c = lerp(color, color * red, saturate(mask.x * strength.x));
    c = lerp(c, c * green, saturate(mask.y * strength.y));
    return lerp(c, c * blue, saturate(mask.z * strength.z));
}

float3 rikka_npr_line(
    float sdf, float distance, float thickness, float distance_scale, float largest, float far, float smoothness,
    float3 color, float strength, float3 albedo, out float line)
{
    float threshold = min(distance_scale * distance + thickness, min(largest, 0.99));
    float soft = distance > far ? smoothness : 0.0;
    line = soft > 0.0 ? smoothstep(threshold - soft, threshold + soft, sdf) : step(threshold, sdf);
    return albedo + line * (color * albedo - albedo) * strength;
}

struct RikkaNprLine {
    float3 color;
    float line;
};

RikkaNprLine rikka_npr_sdf_line(
    float sdf, float distance, float thickness, float distance_scale, float largest, float far, float smoothness,
    float3 color, float strength, float3 albedo)
{
    RikkaNprLine l;
    l.color = rikka_npr_line(sdf, distance, thickness, distance_scale, largest, far, smoothness, color, strength, albedo, l.line);
    return l;
}

float rikka_npr_band(float distance, float width) {
    float band = clamp(1.0 - abs(distance) / max(width, 1e-3), 0.0, 1.0);
    return band * band;
}

float3 rikka_npr_scatter_bands(
    float facing, float edge, float width, float deep_width, float deep_strength, float lit_side, float3 color,
    float3 deep_color)
{
    float boundary = facing >= edge
        ? (facing - edge) / max(1.0 - edge, 1e-3)
        : (facing - edge) / max(1.0 + edge, 1e-3);
    float deep_reach = max(deep_width, width);
    float shallow = rikka_npr_band(boundary, width);
    float deep = rikka_npr_band(boundary, deep_reach);
    float shadow_side = clamp(-boundary / max(deep_reach, 1e-3), 0.0, 1.0);
    return (color * shallow + deep_color * deep * deep_strength) * lerp(lit_side, 1.0, shadow_side);
}
