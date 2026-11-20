__exported import rikka.npr.rim;

struct RikkaNprScreen {
    float2 uv;
    float2 pixel;
    float depth;
    float focal;
    float height;
    float aspect;
};

RikkaNprScreen rikka_npr_screen(RikkaPixel pixel, RikkaContext context) {
    RikkaNprScreen s;
    s.pixel = pixel.frag_coord.xy;
    s.uv = pixel.frag_coord.xy * context.view.inverse_viewport;
    s.depth = pixel.view_depth;
    s.focal = rikka_npr_focal(context) / RIKKA_NPR_REFERENCE_FOCAL;
    s.height = context.view.viewport.y;
    s.aspect = context.view.viewport.x / max(context.view.viewport.y, 1.0);
    return s;
}

struct RikkaNprSceneDepth {
    float depth;
    float gap;
};

RikkaNprSceneDepth rikka_npr_scene_depth(RikkaPixel pixel, RikkaContext context, float2 offset, float source) {
    RikkaNprSceneDepth d;
    uint image = rikka_npr_depth_image(context, source);
    d.depth = image == RIKKA_NONE ? 1e6 : rikka_npr_read_depth(context, image, pixel.frag_coord.xy + offset);
    d.gap = d.depth - pixel.view_depth;
    return d;
}

struct RikkaNprHairShadow {
    float shadow;
    float lit;
};

RikkaNprHairShadow rikka_npr_hair_shadow(
    RikkaPixel pixel, RikkaContext context, float3 light, float distance, float reach, float softness)
{
    RikkaNprHairShadow h;
    h.shadow = rikka_hair_shadow(context, pixel.world_position, pixel.view_depth, light, distance, reach, softness);
    h.lit = 1.0 - h.shadow;
    return h;
}

float rikka_npr_see_through(RikkaPixel pixel, RikkaContext context, float reach) {
    return rikka_see_through(context, pixel, reach);
}

struct RikkaNprCurvature {
    float mean;
    float magnitude;
};

RikkaNprCurvature rikka_npr_curvature(RikkaPixel pixel, float3 normal, float3 position) {
    float3 nx = ddx(normal);
    float3 ny = ddy(normal);
    float3 px = ddx(position);
    float3 py = ddy(position);
    float along_x = dot(nx, px) / max(dot(px, px), 1e-12);
    float along_y = dot(ny, py) / max(dot(py, py), 1e-12);
    float kept = pixel.hull ? 0.0 : 1.0;
    RikkaNprCurvature c;
    c.mean = 0.5 * (along_x + along_y) * kept;
    c.magnitude = length(abs(nx) + abs(ny)) / max(length(abs(px) + abs(py)), 1e-6) * kept;
    return c;
}

float rikka_npr_skin_likeness(float3 linear) {
    float3 c = pow(max(linear, float3(0.0)), float3(1.0 / 2.2));
    float luma = dot(c, float3(0.299, 0.587, 0.114));
    float cb = 0.5 + (c.b - luma) * 0.564;
    float cr = 0.5 + (c.r - luma) * 0.713;
    float blue = smoothstep(0.40, 0.42, cb) * (1.0 - smoothstep(0.47, 0.485, cb));
    float red = smoothstep(0.53, 0.545, cr) * (1.0 - smoothstep(0.61, 0.63, cr));
    return blue * red * smoothstep(0.68, 0.76, luma);
}

float rikka_npr_skin_reject(RikkaContext context, float3 color, float amount) {
    if ((context.pass & RIKKA_PASS_SEE_THROUGH) == 0u) {
        return 1.0;
    }
    return 1.0 - saturate(amount) * rikka_npr_skin_likeness(color);
}
