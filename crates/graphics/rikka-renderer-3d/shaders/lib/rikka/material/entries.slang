__exported import rikka.material.model;
__exported import rikka.depth;
__exported import rikka.characters;
import rikka.temporal;

float rikka_shown<M : IShadingModel>(__constref M model, RikkaContext context, RikkaPixel pixel) {
    if ((context.pass & RIKKA_PASS_SEE_THROUGH) == 0u) {
        return 1.0;
    }
    RikkaSeeThrough see = model.seen_through(context);
    float shown = see.amount > 0.0 ? see.amount * rikka_see_through(context, pixel, see.reach) : 0.0;
    if (shown <= 0.0) {
        discard;
    }
    return shown;
}

float4 rikka_forward<M : IShadingModel, S : ISurface<M.Data>>(
    __constref S surface, __constref M model, RikkaContext context, RikkaPixel pixel, float shown)
{
    float4 color = model.shade(pixel, surface.evaluate(pixel, context), context);
    color.a *= shown;
    return color;
}

float3 rikka_smoothed_normal(RikkaInstance inst, RikkaVertex v) {
    if (inst.outline_normals == 0ul) {
        return v.normal;
    }
    uint packed = ((RikkaIndexListRef)inst.outline_normals)[v.vertex_index - inst.vertex_offset];
    float3 local = rikka_decode_octahedral(packed);
    float3 n = normalize(v.normal);
    float3 t = v.tangent.xyz - n * dot(n, v.tangent.xyz);
    float length_of = length(t);
    if (length_of < 1e-5) {
        return n;
    }
    t /= length_of;
    float3 b = cross(n, t) * v.tangent.w;
    return normalize(t * local.x + b * local.y + n * local.z);
}

struct RikkaEmissiveShade {
    float4 color : SV_Target0;
    float4 emission : SV_Target1;
};

RikkaEmissiveShade rikka_forward_emissive<M : IShadingModel, S : ISurface<M.Data>>(
    __constref S surface, __constref M model, RikkaContext context, RikkaPixel pixel)
{
    RikkaSurfaceData<M.Data> data = surface.evaluate(pixel, context);
    RikkaEmissiveShade shaded;
    shaded.color = model.shade(pixel, data, context);
    shaded.emission = float4(max(model.emitted(data, context), float3(0.0)) * shaded.color.a, shaded.color.a);
    return shaded;
}

RikkaScatter rikka_scattered<M : IShadingModel, S : ISurface<M.Data>>(
    __constref S surface, __constref M model, RikkaContext context, RikkaVaryings varyings, float4 frag_coord, bool front_facing)
{
    RikkaPixel pixel = rikka_pixel(context, varyings, frag_coord, front_facing, false);
    return model.scattered(pixel, surface.evaluate(pixel, context), context);
}

RikkaHullVaryings rikka_hull_vertex<M : IShadingModel, S : ISurface<M.Data>>(
    __constref S surface, __constref M model, RikkaContext context, uint instance_index, uint vertex_index, out float4 position)
    where optional S : IVertex
{
    RikkaInstance inst = context.instance;
    RikkaViewRef view = context.view;

    float3 local = ((RikkaPositionRef)inst.positions)[vertex_index];
    float4 world = mul(inst.transform, float4(local, 1.0));
    RikkaVertex v = rikka_vertex(inst, vertex_index, local, world);
    rikka_deform<S>(surface, v, context, context.time);
    world.xyz = v.world_position;

    RikkaHullVertex hull;
    hull.world_position = world.xyz;
    hull.normal = normalize(v.normal);
    hull.smoothed_normal = rikka_smoothed_normal(inst, v);
    hull.tangent = float4(normalize(v.tangent.xyz), v.tangent.w);
    hull.uv = v.uv;
    hull.color = v.color;
    RikkaHull shape = surface.hull(hull, context, model.hull_scale(context));
    float width = shape.width;
    world.xyz += normalize(shape.direction) * width;
    if (shape.depth_offset != 0.0) {
        world.xyz += normalize(world.xyz - view.camera_position) * shape.depth_offset;
    }

    RikkaHullVaryings out;
    out.world_position = world.xyz;
    out.normal = v.normal;
    out.tangent = v.tangent;
    out.uv = v.uv;
    out.color = v.color;
    out.instance = instance_index;
    out.surface_position = hull.world_position;

    position = mul(view.view_projection, world);
    position.z -= 1e-6 * position.w;
    if (width <= 0.0) {
        position = float4(0.0);
    }
    return out;
}

void rikka_cut<M : IShadingModel, S : ISurface<M.Data>>(__constref S surface, RikkaContext context, float2 uv) {
    RikkaCoverage coverage;
    coverage.uv = uv;
    if (surface.coverage(coverage, context) < context.material.alpha_cutoff) {
        discard;
    }
}

uint4 rikka_surfaces<M : IShadingModel, S : ISurface<M.Data>>(
    __constref S surface, __constref M model, RikkaContext context, RikkaSurfaceVaryings varyings, bool front_facing)
{
    if (RIKKA_CLIPPING) {
        RikkaViewRef view = context.view;
        float4 clip = varyings.clip;
        float4 world = mul(view.inverse_view_projection, float4(clip.xy + view.jitter * clip.w, clip.zw));
        rikka_clip(context.frame, world.xyz / world.w);
    }
    if (RIKKA_CUTOUT) {
        rikka_cut<M, S>(surface, context, varyings.uv);
    }
    RikkaGBuffer g = model.gbuffer(context);

    float3 normal = normalize(varyings.normal);
    if ((context.instance.flags & RIKKA_DOUBLE_SIDED) != 0u && !front_facing) {
        normal = -normal;
    }
    float2 velocity = rikka_motion(varyings.clip, varyings.previous_clip);

    float film = smoothstep(0.35, 1.0, clamp(varyings.wet, 0.0, 1.0));
    float roughness = lerp(g.roughness, min(g.roughness, RIKKA_WET_FILM_ROUGHNESS), film);
    float reflection = max(clamp(g.reflection, 0.0, 1.0), sqrt(RIKKA_WET_FILM_F0) * film);

    return uint4(
        rikka_encode_octahedral(normal),
        packHalf2x16(velocity),
        g.figure,
        packUnorm4x8(float4(roughness, reflection, 0.0, 0.0)));
}

RikkaVaryings rikka_vertex_forward<M : IShadingModel, S : ISurface<M.Data>>(
    __constref S surface, RikkaContext context, uint instance_index, uint vertex_index, out float4 position)
    where optional S : IVertex
{
    RikkaInstance inst = context.instance;
    RikkaVertex v = rikka_hooked_vertex<S>(surface, context, vertex_index);
    float4 world = float4(v.world_position, 1.0);

    RikkaVaryings out;
    out.world_position = world.xyz;
    out.normal = v.normal;
    out.tangent = v.tangent;
    out.uv = v.uv;
    out.color = v.color;
    out.instance = instance_index;
    out.wet = rikka_wet_of(inst, vertex_index);
    position = mul(context.view.view_projection, world);
    return out;
}

RikkaDepthVaryings rikka_vertex_depth<M : IShadingModel, S : ISurface<M.Data>>(
    __constref S surface, RikkaContext context, uint instance_index, uint vertex_index, out float4 position)
    where optional S : IVertex
{
    RikkaVertex v = rikka_hooked_vertex<S>(surface, context, vertex_index);
    float4 world = float4(v.world_position, 1.0);

    RikkaDepthVaryings out;
    out.uv = RIKKA_CUTOUT ? v.uv : float2(0.0);
    out.instance = instance_index;
    out.world_position = world.xyz;
    float4x4 view_projection = (context.pass & RIKKA_PASS_SHADOW) != 0u
        ? context.frame.shadows[rikka_push.slot].view_projection
        : context.view.view_projection;
    position = mul(view_projection, world);
    return out;
}

RikkaSurfaceVaryings rikka_vertex_surfaces<M : IShadingModel, S : ISurface<M.Data>>(
    __constref S surface, RikkaContext context, uint instance_index, uint vertex_index, out float4 position)
    where optional S : IVertex
{
    RikkaInstance inst = context.instance;
    RikkaScene scene = context.frame.look.scene;
    float3 local = ((RikkaPositionRef)inst.positions)[vertex_index];
    float3 previous_local = local;
    if (scene.history != 0u && (inst.flags & RIKKA_DEFORMED) != 0u) {
        previous_local = context.frame.previous_positions[vertex_index];
    }
    float4 world = mul(inst.transform, float4(local, 1.0));
    float4 previous_world = mul(context.frame.previous_transforms[instance_index], float4(previous_local, 1.0));
    RikkaVertex v = rikka_vertex(inst, vertex_index, local, world);
    rikka_deform<S>(surface, v, context, context.time);
    world.xyz = v.world_position;
    RikkaVertex previous = rikka_vertex(inst, vertex_index, previous_local, previous_world);
    rikka_deform<S>(surface, previous, context, scene.previous_time);
    previous_world.xyz = previous.world_position;

    RikkaSurfaceVaryings out;
    out.normal = v.normal;
    out.uv = v.uv;
    out.clip = mul(context.view.view_projection, world);
    position = out.clip;
    out.clip = rikka_unjittered(context.view, out.clip);
    out.previous_clip = mul(scene.previous_view_projection, previous_world);
    out.instance = instance_index;
    out.wet = rikka_wet_of(inst, vertex_index).a;
    return out;
}

struct RikkaLayeredSurface {
    uint4 surface: SV_Target0;
    uint layers: SV_Target1;
};
