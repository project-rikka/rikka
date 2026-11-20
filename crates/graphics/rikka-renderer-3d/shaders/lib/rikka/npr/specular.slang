__exported import rikka.npr.head;

struct RikkaNprStepped {
    float value;
    float term;
    float revealed;
};

RikkaNprStepped rikka_npr_stepped(
    float3 normal, float3 light, float3 view, float shininess, float threshold, float intensity, float softness, float offset)
{
    float3 h = normalize(light + view);
    RikkaNprStepped s;
    s.term = pow(max(dot(normal, h), 0.001), max(shininess, 1e-3));
    float edge = offset - threshold;
    float soft = max(softness, 0.0);
    s.revealed = soft > 1e-5 ? smoothstep(edge - soft, edge + soft, s.term) : step(edge, s.term);
    s.value = s.revealed * intensity;
    return s;
}

float3 rikka_npr_toon_ggx(float3 normal, float3 light, float3 view, float roughness, float3 f0, float gain) {
    float3 h = normalize(light + view);
    float d = rikka_ggx_distribution(max(dot(normal, h), 0.0), clamp(roughness, 0.02, 1.0));
    float3 f = rikka_fresnel(f0, max(dot(view, h), 0.0));
    return f * (gain > 0.0 ? saturate(gain * d) : d);
}

float2 rikka_npr_matcap_uv(RikkaContext context, float3 normal, float3 view, float mode, float tile, float flip) {
    float2 at;
    switch (int(mode)) {
    case 1: {
        float3 n = normalize(rikka_npr_to_view(context, normal));
        at = n.xy;
        break;
    }
    case 2: {
        float3 n = normalize(rikka_npr_to_view(context, normal));
        float3 up = rikka_npr_to_view(context, float3(0.0, 1.0, 0.0));
        float roll = atan2(up.x, up.y);
        float c = cos(roll);
        float s = sin(roll);
        at = float2(c * n.x - s * n.y, s * n.x + c * n.y);
        break;
    }
    default: {
        float3 across = normalize(float3(view.z, 0.0, -view.x));
        float3 up = cross(view, across);
        at = float2(dot(across, normal), dot(up, normal));
        break;
    }
    }
    at.x *= tile;
    float2 uv = at * 0.495 + 0.5;
    return flip > 0.5 ? float2(uv.x, 1.0 - uv.y) : uv;
}

float3 rikka_npr_overlay(float3 base, float3 blend) {
    return lerp(2.0 * base * blend, 1.0 - 2.0 * (1.0 - base) * (1.0 - blend), step(0.5, base));
}

struct RikkaNprMatcap {
    float3 color;
    float3 matcap;
    float alpha;
};

RikkaNprMatcap rikka_npr_matcap(
    float3 base, RikkaTexture map, float2 uv, float3 tint, float mask, float strength, float mode,
    float lod, float2 scroll, float time)
{
    float4 texel = map.sampled_level(uv + scroll * time, max(lod, 0.0));
    RikkaNprMatcap m;
    m.matcap = texel.rgb * tint;
    m.alpha = texel.a;
    float w = saturate(strength * mask * texel.a);
    switch (int(mode)) {
    case 1:
        m.color = lerp(base, base * m.matcap, w);
        break;
    case 2:
        m.color = lerp(base, m.matcap, w);
        break;
    case 3:
        m.color = lerp(base, rikka_npr_overlay(base, m.matcap), w);
        break;
    case 4:
        m.color = lerp(base, 1.0 - (1.0 - base) * (1.0 - m.matcap), w);
        break;
    default:
        m.color = base + m.matcap * w;
        break;
    }
    return m;
}

float3 rikka_npr_metal(
    float3 albedo, float value, float3 dark, float3 light, float brightness, float term, float sharp_edge,
    float3 sharp_color, float3 specular_color, float intensity, float shadow, float shadow_specular, float3 shadow_color)
{
    float m = saturate(value * brightness);
    float3 metal = albedo * lerp(dark, light, m);
    float p = saturate(term);
    float3 specular = p > sharp_edge ? sharp_color : p * specular_color * intensity;
    float t = saturate(shadow);
    specular = lerp(specular, specular * shadow_specular, t);
    return (metal + 0.5 * specular) * lerp(float3(1.0), shadow_color, t);
}

float rikka_npr_packed_matcap(float4 texel, float shine, float roughness) {
    float value = lerp(texel.r, texel.g, saturate(shine));
    value = lerp(value, texel.b, saturate(3.0 * roughness - 1.0));
    return lerp(value, texel.a, saturate(3.0 * roughness - 2.0));
}

float rikka_npr_kajiya_lobe(float3 tangent, float3 normal, float3 h, float shift, float exponent) {
    float3 axis = normalize(tangent + normal * shift);
    float along = dot(axis, h);
    float sine = sqrt(max(1.0 - along * along, 0.0));
    return pow(sine, max(exponent, 1e-3)) * smoothstep(-1.0, 0.0, along);
}

struct RikkaNprKajiya {
    float3 color;
    float primary;
    float secondary;
};

RikkaNprKajiya rikka_npr_kajiya(
    float3 tangent, float3 normal, float3 light, float3 view, float shift, float second_shift, float exponent,
    float second_exponent, float strength, float second_strength, float3 second_color)
{
    float3 strand = tangent - normal * dot(tangent, normal);
    strand = dot(strand, strand) > 1e-8 ? normalize(strand) : tangent;
    float3 h = normalize(light + view);
    RikkaNprKajiya k;
    k.primary = rikka_npr_kajiya_lobe(strand, normal, h, shift, exponent) * strength;
    k.secondary = rikka_npr_kajiya_lobe(strand, normal, h, second_shift, second_exponent) * second_strength;
    k.color = float3(k.primary) + second_color * k.secondary;
    return k;
}

float rikka_npr_strand_band(
    float gradient, float jitter, float jitter_scale, float3 tangent, float3 light, float3 view, float offset, float shift,
    float width, float softness)
{
    float3 h = normalize(light + view);
    float position = gradient + (jitter - 0.5) * jitter_scale;
    float center = offset + dot(normalize(tangent), h) * shift;
    float half_width = max(width, 1e-4);
    return 1.0 - smoothstep(half_width * (1.0 - saturate(softness)), half_width, abs(position - center));
}

struct RikkaNprSphereBand {
    float band;
    float3 normal;
};

RikkaNprSphereBand rikka_npr_sphere_band(
    RikkaContext context, float3 position, float3 normal, float3 light, float3 view, float mask, float softness, float falloff)
{
    RikkaNprHead head = rikka_npr_head(context);
    float3 offset = position - head.center;
    float distance = length(offset);
    float blend = head.present > 0.5 ? 1.0 - saturate(falloff * (distance - head.radius)) : 0.0;
    float3 around = distance > 1e-5 ? offset / distance : normal;
    RikkaNprSphereBand b;
    b.normal = normalize(lerp(normal, around, blend));
    float3 h = normalize(light + view);
    float a = sqrt(saturate(rikka_npr_half(dot(light, b.normal))));
    float c = saturate(rikka_npr_half(dot(b.normal, h)));
    b.band = saturate((mask - (1.0 - a * c)) / max(softness, 1e-4));
    return b;
}

float rikka_npr_mask_band(float mask, float3 normal, float3 light, float3 view, float strength) {
    float s = saturate(dot(normal, normalize(light + view)));
    return s * strength * (max(mask, 1.0 - s) - (1.0 - s)) / max(s, 1e-4);
}

float2 rikka_npr_ring_uv(RikkaContext context, float3 normal, float2 uv, float offset_u, float offset_v) {
    float3 n = normalize(rikka_npr_to_view(context, normal));
    float u = lerp(n, float3(0.0, 0.0, 1.0), offset_u).x * 0.5 + 0.5;
    float v = lerp(uv.y, n.y * 0.5 + 0.5, offset_v);
    return float2(u, v);
}

struct RikkaNprStocking {
    float3 color;
    float density;
};

RikkaNprStocking rikka_npr_stocking(
    float mode, float3 albedo, float3 normal, float3 view, float3 range, float3 color, float3 skin_color, float3 dark_color,
    float denier, float power, float sheen_power, float dark_width, float roughness, float shift)
{
    float3 v = int(mode) == 2 ? normalize(view + float3(0.0, shift, 0.0)) : view;
    float facing = max(dot(normal, v), 0.001);
    RikkaNprStocking s;
    switch (int(mode)) {
    case 1: {
        float area = range.x > 0.001 ? 1.0 : 0.0;
        float tile = lerp(1.0, range.z * 0.5 + 0.5, roughness);
        float sp = max(0.039, power);
        float edge = smoothstep(sp, dark_width * sp, facing) * sp;
        s.density = range.x * area * edge;
        float3 dark = albedo * (s.density * (albedo * (1.0 - s.density * dark_color) - 1.0) + 1.0);
        float sheen = saturate(max(0.004, pow(facing, sheen_power)) * (1.0 - sp) * range.y * tile);
        s.color = lerp(dark, color, sheen);
        break;
    }
    case 2: {
        float light = min(pow(facing, power), 1.0);
        float shadow = 1.0 - min(pow(facing, sheen_power), 1.0);
        s.density = shadow;
        float3 lifted = lerp(albedo, albedo * color, light * (1.0 - range.z));
        s.color = lerp(lifted, lifted * dark_color, shadow * range.x);
        break;
    }
    case 3: {
        s.density = 1.0 - smoothstep(denier, dark_width, facing);
        s.color = lerp(albedo * color, albedo * dark_color, s.density * range.x);
        break;
    }
    default: {
        s.density = lerp(denier, 1.0, pow(1.0 - facing, power)) * range.x;
        s.color = lerp(skin_color, albedo * color, s.density);
        break;
    }
    }
    return s;
}
