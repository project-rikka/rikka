import rikka.push;
import rikka.shadow;
import rikka.fluid.style;

struct SprayVaryings {
    [[vk::location(0)]] float2 corner;
    [[vk::location(1)]] nointerpolation float3 light;
    [[vk::location(2)]] nointerpolation float opacity;
    [[vk::location(3)]] nointerpolation float distance;
    [[vk::location(4)]] nointerpolation uint kind;
    [[vk::location(5)]] nointerpolation float softness;
};

static const float2 CORNERS[6] = {
    float2(-1.0, -1.0), float2(1.0, -1.0), float2(1.0, 1.0),
    float2(-1.0, -1.0), float2(1.0, 1.0), float2(-1.0, 1.0) };

static const uint FOAM = 1u;
static const uint BUBBLE = 2u;
static const float SIZES[3] = { 0.45, 1.1, 0.4 };
static const float OPACITIES[3] = { 0.8, 0.85, 0.5 };
static const float SHUTTER = 1.0 / 60.0;

float3 incident(RikkaFrameRef frame, RikkaViewRef view, float3 position, float3 eye, float view_depth,
                bool droplet) {
    RikkaFluidStyle style = rikka_fluid_style(frame);
    float3 lit = float3(0.0);
    for (uint i = 0u; i < view.directional_count; ++i) {
        RikkaLight light = frame.lights[i];
        float3 l = -light.direction;
        float shadow = 1.0;
        if (light.shadow != RIKKA_NONE) {
            uint cascade = rikka_cascade(frame, light, view_depth);
            shadow = rikka_shadow_lookup(frame, frame.shadows[light.shadow + cascade], position, l);
        }
        float facing = 0.5 + 0.5 * dot(eye, l);
        float glint = droplet ? 2.0 * pow(max(dot(-eye, l), 0.0), 8.0) : 0.0;
        float shadow_cel = rikka_fluid_cel_shadow(style, shadow);
        float3 anime = lerp(style.dark, float3(1.0), shadow_cel) * facing + float3(shadow_cel * glint);
        float3 physical = float3(shadow * (facing + glint));
        lit += light.color * light.intensity * lerp(physical, anime, style.stylized)
            * (1.0 / RIKKA_PI);
    }
    float3 sky = 0.5 * (rikka_sky_irradiance(frame, float3(0.0, 1.0, 0.0)) + rikka_sky_irradiance(frame, eye));
    return lit + lerp(sky, rikka_fluid_anime_sky(frame, style, sky), style.stylized);
}

[shader("vertex")]
SprayVaryings vertex_main(uint vertex_index: SV_VulkanVertexID, out float4 position: SV_Position) {
    SprayVaryings out = {};
    position = float4(-2.0, -2.0, 0.0, 1.0);
    RikkaFrameRef frame = rikka_push.frame;
    RikkaFluidRef fluid = frame.fluid;
    RikkaFluidLayer layer = fluid.layers[rikka_push.slot];
    uint index = vertex_index / 6u;
    if (index >= layer.spray_count) {
        return out;
    }
    RikkaFluidSpray p = ((RikkaFluidSprayRef)layer.spray)[index];
    float life = p.position.w;
    if (!(life > 0.0) || any(isnan(p.position.xyz)) || any(isinf(p.position.xyz))) {
        return out;
    }
    life = min(life, 1.0);
    uint packed = asuint(p.velocity.w);
    uint kind = min(packed & 0xffu, BUBBLE);
    uint substance = packed >> 8u;
    RikkaFluidMaterial material = fluid.materials[
        layer.material_first + min(substance, max(layer.material_count, 1u) - 1u)];

    RikkaViewRef view = frame.view;
    float3 center = p.position.xyz;
    float3 eye = rikka_fluid_eye(view, center);
    float size = layer.radius * SIZES[kind] * lerp(0.5, 1.0, sqrt(life));
    float3 motion = p.velocity.xyz - eye * dot(p.velocity.xyz, eye);
    float streak = length(motion) * SHUTTER;
    float3 along = streak > 1e-6 ? motion / length(motion)
        : normalize(cross(eye, abs(eye.y) < 0.99 ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0)));
    float3 side = normalize(cross(along, eye));
    float length_along = size + 0.5 * streak;
    float2 corner = CORNERS[vertex_index % 6u];
    float3 world = center + along * (corner.x * length_along) + side * (corner.y * size);

    float view_depth = dot(center - view.camera_position, view.camera_forward);
    float3 tint = lerp(float3(0.92), material.albedo, 1.0 - exp(-material.scattering * 0.005));
    out.corner = corner;
    out.light = tint * incident(frame, view, center, eye, view_depth, kind == 0u);
    out.opacity = OPACITIES[kind] * smoothstep(0.0, 0.2, life) * (size / length_along);
    out.distance = view_depth;
    out.kind = kind;
    out.softness = 2.0 * size;
    position = mul(view.view_projection, float4(world, 1.0));
    return out;
}

[shader("fragment")]
float4 fragment_main(SprayVaryings varyings, float4 frag_coord: SV_Position) : SV_Target {
    float reach = dot(varyings.corner, varyings.corner);
    if (reach >= 1.0) {
        discard;
    }
    float shape = varyings.kind == BUBBLE
        ? lerp(0.25, 1.0, smoothstep(0.55, 0.95, reach)) * (1.0 - smoothstep(0.9, 1.0, reach))
        : (1.0 - reach) * (1.0 - reach);
    RikkaFrameRef frame = rikka_push.frame;
    RikkaFluidRef fluid = frame.fluid;
    RikkaViewRef view = frame.view;
    int2 pixel = int2(frag_coord.xy);
    float depth = rikka_fetch(rikka_images[fluid.scene_depth], pixel, 0).r;
    float surface = depth > 0.0 ? rikka_fluid_distance_of(view.projection, depth) : 1e30;
    float liquid = rikka_fetch(rikka_images[fluid.smoothed], pixel, 0).r;
    if (varyings.kind != BUBBLE && liquid > 0.0) {
        surface = min(surface, liquid + (varyings.kind == FOAM ? 0.5 * varyings.softness : 0.0));
    }
    float fade = clamp((surface - varyings.distance) / max(varyings.softness, 1e-4), 0.0, 1.0);
    float alpha = varyings.opacity * shape * fade;
    if (alpha <= 1e-4) {
        discard;
    }
    return float4(varyings.light * alpha, alpha);
}
