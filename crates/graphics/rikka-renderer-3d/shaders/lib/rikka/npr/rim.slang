__exported import rikka.npr.head;

static const float RIKKA_NPR_REFERENCE_HEIGHT = 1080.0;
static const float RIKKA_NPR_REFERENCE_FOCAL = 2.414;

uint rikka_npr_depth_image(RikkaContext context, float source) {
    RikkaScene scene = context.frame.look.scene;
    switch (int(source)) {
    case 1:
        return scene.hair;
    case 2:
        return scene.face;
    default:
        return scene.depth;
    }
}

float rikka_npr_read_depth(RikkaContext context, uint image, float2 at) {
    int2 texel = clamp(int2(at), int2(0), int2(context.view.viewport) - 1);
    float depth = rikka_read_depth(image, texel);
    return depth <= 0.0 ? 1e6 : rikka_linear_depth(context.view, depth);
}

float2 rikka_npr_screen_direction(RikkaContext context, float3 position, float3 direction, float depth) {
    RikkaViewRef view = context.view;
    float4 here = mul(view.view_projection, float4(position, 1.0));
    float4 along = mul(view.view_projection, float4(position + direction * (0.02 * max(depth, 0.1)), 1.0));
    float2 moved = (along.xy / along.w - here.xy / here.w) * view.viewport;
    float span = length(moved);
    return span > 1e-5 ? moved / span : float2(0.0);
}

struct RikkaNprDepthRim {
    float mask;
    float gap;
};

RikkaNprDepthRim rikka_npr_depth_rim(
    RikkaPixel pixel, RikkaContext context, float3 normal, float3 toward, float width, float direction, float scaling,
    float source, float low, float high, float scale)
{
    RikkaNprDepthRim r;
    r.mask = 0.0;
    r.gap = 0.0;
    uint image = rikka_npr_depth_image(context, source);
    if (image == RIKKA_NONE || width <= 0.0) {
        return r;
    }
    int mode = int(direction);
    float2 step_dir = rikka_npr_screen_direction(context, pixel.world_position, mode == 2 ? toward : normal, pixel.view_depth);
    if (mode == 0) {
        step_dir = float2(step_dir.x >= 0.0 ? 1.0 : -1.0, 0.0);
    }
    float figure = max(scale, 1e-4);
    float relative = max(pixel.view_depth / figure, 0.05);
    float pixels = width * context.view.viewport.y / RIKKA_NPR_REFERENCE_HEIGHT;
    float focal = rikka_npr_focal(context) / RIKKA_NPR_REFERENCE_FOCAL;
    switch (int(scaling)) {
    case 1:
        pixels *= focal / relative;
        break;
    case 2:
        pixels *= focal * rsqrt(relative);
        break;
    default:
        break;
    }
    float beyond = rikka_npr_read_depth(context, image, pixel.frag_coord.xy + step_dir * pixels);
    r.gap = (beyond - pixel.view_depth) / figure;
    r.mask = smoothstep(low, max(high, low + 1e-5), r.gap);
    return r;
}

float rikka_npr_fresnel_rim(float3 normal, float3 view, float power, float lift) {
    return pow(saturate(1.0 - dot(normal, view) + lift), max(power, 1e-5));
}

struct RikkaNprSplitRim {
    float front;
    float back;
};

RikkaNprSplitRim rikka_npr_split_rim(float3 normal, float3 view, float3 light, float power) {
    float3 away = view - light;
    float3 r = dot(away, away) > 1e-8 ? normalize(away) : view;
    float facing = dot(normal, r);
    RikkaNprSplitRim s;
    s.front = saturate(pow(saturate(1.0 - facing), power));
    s.back = saturate(pow(saturate(facing), power));
    return s;
}

float rikka_npr_light_side(float facing, float low, float high, float invert, float mode, float rim, float level) {
    float m = int(mode) == 1
        ? saturate(rim - ((1.0 - rikka_npr_half(facing)) + level))
        : smoothstep(low, max(high, low + 1e-5), facing);
    return invert > 0.5 ? 1.0 - m : m;
}

float3 rikka_npr_rim_shadow(
    RikkaContext context, float3 normal, float3 view, float3 offset, float power, float width, float feather,
    float intensity, float3 color)
{
    float3 n = normalize(rikka_npr_to_view(context, normal));
    float3 v = rikka_npr_to_view(context, view) - offset;
    float3 toward = dot(v, v) > 1e-8 ? normalize(v) : float3(0.0, 0.0, 1.0);
    float r = saturate(dot(toward, n));
    float shadow = smoothstep(feather, 1.0, saturate(pow(max(1.0 - r, 0.001), power) * width)) * intensity * 0.25;
    return lerp(float3(1.0), color * 2.0, shadow);
}

float3 rikka_npr_rim_combine(float3 base, float rim, float3 color, float3 lighting, float light_mix, float mode) {
    float3 added = rim * color * lerp(float3(1.0), lighting, light_mix);
    switch (int(mode)) {
    case 1:
        return base - added;
    case 2:
        return 1.0 - (1.0 - base) * (1.0 - saturate(added));
    default:
        return base + added;
    }
}
