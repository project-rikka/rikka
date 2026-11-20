import rikka.push;
import rikka.fluid;
import rikka.fluid.splat;
import rikka.temporal;

static const float2 CORNERS[6] = {
    float2(0.0, 0.0), float2(1.0, 0.0), float2(1.0, 1.0),
    float2(0.0, 0.0), float2(1.0, 1.0), float2(0.0, 1.0) };

static const float ELLIPSOID_VOLUME = 4.18879020;

[shader("vertex")]
RikkaFluidSplat vertex_main(uint vertex_index: SV_VulkanVertexID, out float4 position: SV_Position) {
    RikkaFluidSplat out = {};
    position = float4(-2.0, -2.0, 0.0, 1.0);
    RikkaFluidRef fluid = rikka_push.frame.fluid;
    RikkaFluidLayer layer = fluid.layers[rikka_push.slot];
    RikkaFluidView view = fluid.views[rikka_push.flags];
    uint particle = vertex_index / 6u;
    if (particle >= layer.count) {
        return out;
    }
    RikkaFluidParticle p = ((RikkaFluidParticleRef)layer.particles)[particle];
    float3 radii = p.radii.xyz;
    float3 center = p.center.xyz;
    bool alive = min(radii.x, min(radii.y, radii.z)) > 0.0
        && max(radii.x, max(radii.y, radii.z)) < 1e4
        && !any(isnan(center)) && !any(isinf(center));
    if (!alive) {
        return out;
    }

    float3x3 axes = rikka_fluid_rotation(p.rotation);
    RikkaFluidBounds bounds;
    if (!rikka_fluid_bounds(view.view_projection, center, axes, radii, bounds)) {
        return out;
    }
    float2 low = max(bounds.low - view.texel, float2(-1.0));
    float2 high = min(bounds.high + view.texel, float2(1.0));
    if (any(low >= high)) {
        return out;
    }
    float2 ndc = lerp(low, high, CORNERS[vertex_index % 6u]);
    float near_depth = bounds.depth;
    float far_depth = view.perspective != 0u ? near_depth * 0.5 : near_depth - 0.25;
    float near_w;
    float far_w;
    float3 near_point = rikka_fluid_unproject(view.inverse, ndc, near_depth, near_w);
    float3 far_point = rikka_fluid_unproject(view.inverse, ndc, far_depth, far_w);
    float3x3 to_unit = float3x3(
        rikka_column(axes, 0) / radii.x,
        rikka_column(axes, 1) / radii.y,
        rikka_column(axes, 2) / radii.z);

    uint substance = asuint(p.center.w);
    uint material_index = layer.material_first + min(substance, max(layer.material_count, 1u) - 1u);
    RikkaFluidMaterial material = fluid.materials[material_index];

    float2 velocity = float2(0.0);
    if (rikka_push.flags == 0u) {
        float4 now = rikka_unjittered(rikka_push.frame.view, mul(view.view_projection, float4(center, 1.0)));
        float4 before = mul(fluid.previous_view_projection,
            float4(center - p.velocity.xyz * fluid.delta_time, 1.0));
        if (now.w > 0.0 && before.w > 0.0) {
            velocity = (now.xy / now.w - before.xy / before.w) * 0.5;
        }
    }

    out.origin = mul(to_unit, near_point - center);
    out.direction = mul(to_unit, far_point - near_point);
    out.span = far_point - near_point;
    out.clip = float4(near_depth * near_w, near_w, far_depth * far_w, far_w);
    out.front = float4(velocity, material.roughness, material.ior);
    out.absorption = float4(material.absorption, material.extinction);
    out.scattering = float4(material.albedo * material.scattering, material.scattering);
    out.volume = clamp(layer.volume / (ELLIPSOID_VOLUME * radii.x * radii.y * radii.z), 0.0, 8.0);
    position = float4(ndc, near_depth, 1.0);
    return out;
}
