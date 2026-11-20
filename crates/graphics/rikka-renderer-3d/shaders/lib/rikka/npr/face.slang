__exported import rikka.npr.head;

static const float RIKKA_NPR_FACE_DEPTH = 0.25;
static const float2 RIKKA_NPR_NOSE_SIDE = float2(0.2, 0.7);
static const float RIKKA_NPR_NOSE_EDGE = 0.03;
static const float RIKKA_NPR_NOSE_TAIL = 0.5;
static const float RIKKA_NPR_CHIN_FALL = 0.3;

struct RikkaNprFaceSample {
    float sdf;
    float mirrored;
};

RikkaNprFaceSample rikka_npr_face_sample(RikkaPixel pixel, RikkaTexture map, float2 uv, float channel) {
    int picked = clamp(int(channel), 0, 3);
    RikkaNprFaceSample s;
    s.sdf = map.sampled(uv)[picked];
    s.mirrored = map.sampled(float2(1.0 - uv.x, uv.y))[picked];
    return s;
}

struct RikkaNprFace {
    float lit;
    float threshold;
    float sdf;
    float upright;
};

RikkaNprFace rikka_npr_face_sdf(
    RikkaContext context, float sdf, float mirrored_sdf, float3 light, float mirror, float softness, float rotation,
    float cutoff)
{
    RikkaNprHeadLight h = rikka_npr_head_light(context, light);
    bool light_right = h.right > 0.0;
    bool mirrored = int(mirror) == 1 ? light_right : !light_right;
    RikkaNprFace f;
    f.sdf = mirrored ? mirrored_sdf : sdf;
    float threshold = 1.0 - h.facing;
    if (rotation != 0.0) {
        threshold = rikka_npr_linearstep(max(rotation, 0.0), min(rotation + 1.0, 1.0), threshold);
    }
    f.threshold = threshold;
    float soft = max(softness, 0.0);
    f.lit = soft > 1e-5 ? smoothstep(threshold - soft, threshold + soft, f.sdf) : step(threshold, f.sdf);
    if (cutoff > -1.0) {
        f.lit *= step(cutoff, h.forward);
    }
    f.upright = h.upright;
    return f;
}

float rikka_npr_nose_cover(float3 local, float2 light, float strength, float width, float top, float tip) {
    float side = abs(light.x);
    float reach = width * side * smoothstep(RIKKA_NPR_NOSE_SIDE.x, RIKKA_NPR_NOSE_SIDE.y, side);
    float down = (top - local.y) / max(top - tip, 1e-3);
    float spread = reach * max(min(down, (1.0 + RIKKA_NPR_NOSE_TAIL - down) / RIKKA_NPR_NOSE_TAIL), 0.0);
    float away = light.x >= 0.0 ? -local.x : local.x;
    return clamp(strength, 0.0, 1.0) * smoothstep(0.0, RIKKA_NPR_NOSE_EDGE, away)
        * smoothstep(0.0, RIKKA_NPR_NOSE_EDGE, spread - away);
}

struct RikkaNprProceduralFace {
    float lit;
    float value;
    float upright;
};

RikkaNprProceduralFace rikka_npr_face_procedural(
    RikkaContext context, float3 position, float3 light, float width, float wrap, float softness,
    float nose_shadow, float nose_width, float nose_top, float nose_tip, float chin, float chin_height)
{
    RikkaNprHead head = rikka_npr_head(context);
    RikkaNprHeadLight h = rikka_npr_head_light(context, light);
    float3 offset = (position - head.center) / max(head.radius, 1e-3);
    float3 local = float3(dot(offset, head.right), dot(offset, head.up), dot(offset, head.forward));
    float2 flat = float2(h.right, h.forward);
    float2 around = normalize(float2(local.x / max(width, 1e-3), max(local.z, RIKKA_NPR_FACE_DEPTH)));
    float facing = dot(around, flat) + wrap
        + chin * smoothstep(chin_height, chin_height - RIKKA_NPR_CHIN_FALL, local.y);
    RikkaNprProceduralFace f;
    f.value = lerp(facing, -1.0, rikka_npr_nose_cover(local, flat, nose_shadow, nose_width, nose_top, nose_tip));
    float soft = max(softness, 1e-3);
    f.lit = smoothstep(-soft, soft, f.value);
    f.upright = h.upright;
    return f;
}

float rikka_npr_nose_line(RikkaContext context, float mask, float mode, float power, float threshold, float3 normal, float3 view) {
    if (int(mode) == 1) {
        return step(threshold, mask * pow(saturate(dot(normal, view)), max(power, 1e-3)));
    }
    float facing = abs(dot(rikka_npr_head(context).forward, view));
    return step(threshold - mask, pow(facing, max(power, 1e-3)));
}

float rikka_npr_face_highlight(
    float mask, float threshold, float3 normal, float3 light, float3 view, float window, float power, float strength)
{
    float3 h = normalize(light + view);
    float revealed = smoothstep(threshold - window, threshold + window, saturate(mask - 0.5));
    return revealed * pow(saturate(dot(normal, h)), max(power, 1e-3)) * strength;
}

float rikka_npr_eye_through_hair(RikkaContext context, float3 view, float across, float down, float lowest) {
    RikkaNprHead head = rikka_npr_head(context);
    float3 sideways = view - head.up * dot(view, head.up);
    float3 vertical = view - head.right * dot(view, head.right);
    float3 xz = dot(sideways, sideways) > 1e-8 ? normalize(sideways) : head.forward;
    float3 yz = dot(vertical, vertical) > 1e-8 ? normalize(vertical) : head.forward;
    float a = saturate((1.0 - max(0.0, dot(xz, head.forward))) / max(across, 1e-4));
    float b = saturate((1.0 - max(0.0, dot(yz, head.forward))) / max(down, 1e-4));
    return max(max(a, b), lowest);
}

float rikka_npr_outline_fade(RikkaContext context, float3 view, float mask, float power) {
    float facing = max(0.0, dot(rikka_npr_head(context).forward, view));
    return smoothstep(-0.02, 0.0, 1.0 - pow(facing, max(power, 1e-3)) - mask);
}

float2 rikka_npr_eye_parallax(RikkaPixel pixel, float2 uv, float depth, float ior, float mask) {
    float3 n = pixel.geometric_normal;
    float3 t = pixel.tangent.xyz - n * dot(n, pixel.tangent.xyz);
    t = dot(t, t) > 1e-10 ? normalize(t) : (abs(n.y) < 0.999 ? normalize(cross(float3(0.0, 1.0, 0.0), n)) : float3(1.0, 0.0, 0.0));
    float3 b = cross(n, t) * pixel.tangent.w;
    float3 ray = ior > 1.0 ? refract(-pixel.view, n, 1.0 / ior) : -pixel.view;
    float3 local = float3(dot(ray, t), dot(ray, b), dot(ray, n));
    return uv + local.xy / max(abs(local.z), 0.25) * depth * mask;
}
