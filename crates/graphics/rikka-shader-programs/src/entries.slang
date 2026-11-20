import rikka.material.traced;

struct RikkaMaterialBlock {
    Surface surface;
    Model model;
};

[shader("fragment")]
float4 rikka_entry_forward(RikkaVaryings varyings, float4 frag_coord: SV_Position, bool front_facing: SV_IsFrontFace)
    : SV_Target
{
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    RikkaPixel pixel = rikka_pixel(context, varyings, frag_coord, front_facing, false);
    float shown = rikka_shown<Model>(block->model, context, pixel);
    return rikka_forward<Model, Surface>(block->surface, block->model, context, pixel, shown);
}

[shader("fragment")]
float4 rikka_entry_forward_traced(RikkaVaryings varyings, float4 frag_coord: SV_Position, bool front_facing: SV_IsFrontFace)
    : SV_Target
{
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    RikkaPixel pixel = rikka_pixel(context, varyings, frag_coord, front_facing, false);
    float shown = rikka_shown<Model>(block->model, context, pixel);
    pixel.traced = rikka_traced(context, pixel);
    return rikka_forward<Model, Surface>(block->surface, block->model, context, pixel, shown);
}

[shader("vertex")]
RikkaHullVaryings rikka_entry_hull_vertex(
    uint vertex_index: SV_VulkanVertexID,
    uint instance_index: SV_VulkanInstanceID,
    out float4 position: SV_Position)
{
    RikkaContext context = rikka_context(instance_index);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    return rikka_hull_vertex<Model, Surface>(block->surface, block->model, context, instance_index, vertex_index, position);
}

[shader("fragment")]
float4 rikka_entry_hull(RikkaHullVaryings varyings, float4 frag_coord: SV_Position, bool front_facing: SV_IsFrontFace)
    : SV_Target
{
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    RikkaPixel pixel = rikka_pixel(context, varyings.shaded(), frag_coord, front_facing, true);
    return rikka_forward<Model, Surface>(block->surface, block->model, context, pixel, 1.0);
}

[shader("fragment")]
float4 rikka_entry_hull_traced(RikkaHullVaryings varyings, float4 frag_coord: SV_Position, bool front_facing: SV_IsFrontFace)
    : SV_Target
{
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    RikkaPixel pixel = rikka_pixel(context, varyings.shaded(), frag_coord, front_facing, true);
    pixel.traced = rikka_traced_hull(context, pixel, varyings.surface_position);
    return rikka_forward<Model, Surface>(block->surface, block->model, context, pixel, 1.0);
}

[shader("fragment")]
uint rikka_entry_depth(RikkaDepthVaryings varyings) : SV_Target {
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    rikka_cut<Model, Surface>(block->surface, context, varyings.uv);
    return rikka_identity(context.instance);
}

[shader("fragment")]
void rikka_entry_cutout(RikkaDepthVaryings varyings) {
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    rikka_cut<Model, Surface>(block->surface, context, varyings.uv);
}

[shader("fragment")]
uint4 rikka_entry_surfaces(RikkaSurfaceVaryings varyings, bool front_facing: SV_IsFrontFace) : SV_Target {
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    return rikka_surfaces<Model, Surface>(block->surface, block->model, context, varyings, front_facing);
}

[shader("vertex")]
RikkaVaryings rikka_entry_forward_vertex(
    uint vertex_index: SV_VulkanVertexID,
    uint instance_index: SV_VulkanInstanceID,
    out float4 position: SV_Position)
{
    RikkaContext context = rikka_context(instance_index);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    return rikka_vertex_forward<Model, Surface>(block->surface, context, instance_index, vertex_index, position);
}

[shader("vertex")]
RikkaDepthVaryings rikka_entry_depth_vertex(
    uint vertex_index: SV_VulkanVertexID,
    uint instance_index: SV_VulkanInstanceID,
    out float4 position: SV_Position)
{
    RikkaContext context = rikka_context(instance_index);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    return rikka_vertex_depth<Model, Surface>(block->surface, context, instance_index, vertex_index, position);
}

[shader("vertex")]
RikkaSurfaceVaryings rikka_entry_surfaces_vertex(
    uint vertex_index: SV_VulkanVertexID,
    uint instance_index: SV_VulkanInstanceID,
    out float4 position: SV_Position)
{
    RikkaContext context = rikka_context(instance_index);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    return rikka_vertex_surfaces<Model, Surface>(block->surface, context, instance_index, vertex_index, position);
}

[shader("fragment")]
float4 rikka_entry_probe(RikkaVaryings varyings, float4 frag_coord: SV_Position, bool front_facing: SV_IsFrontFace)
    : SV_Target
{
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    RikkaPixel pixel = rikka_pixel(context, varyings, frag_coord, front_facing, false);
    float4 color = rikka_forward<Model, Surface>(block->surface, block->model, context, pixel,
        rikka_shown<Model>(block->model, context, pixel));
    color += rikka_forward<Model, Surface>(block->surface, block->model, context,
        rikka_pixel(context, varyings, frag_coord, front_facing, true), 1.0);
    float4 position;
    RikkaHullVaryings hull = rikka_hull_vertex<Model, Surface>(block->surface, block->model, context, varyings.instance, 0u, position);
    RikkaVaryings moved = rikka_vertex_forward<Model, Surface>(block->surface, context, varyings.instance, 0u, position);
    RikkaSurfaceVaryings surface;
    surface.normal = varyings.normal;
    surface.uv = varyings.uv;
    surface.clip = frag_coord;
    surface.previous_clip = frag_coord;
    surface.instance = varyings.instance;
    surface.wet = 0.0;
    uint4 written = rikka_surfaces<Model, Surface>(block->surface, block->model, context, surface, front_facing);
    rikka_cut<Model, Surface>(block->surface, context, varyings.uv);
    RikkaCoverage coverage;
    coverage.uv = varyings.uv;
    color.rgb += block->surface.transmittance(coverage, context);
    RikkaScatter scattered = rikka_scattered<Model, Surface>(block->surface, block->model, context, varyings, frag_coord, front_facing);
    return color + position + scattered.diffuse + float4(hull.world_position + moved.world_position, float(written.x));
}

[shader("fragment")]
RikkaLayeredSurface rikka_entry_surfaces_layers(RikkaSurfaceVaryings varyings, bool front_facing: SV_IsFrontFace) {
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    RikkaLayeredSurface written;
    written.surface = rikka_surfaces<Model, Surface>(block->surface, block->model, context, varyings, front_facing);
    written.layers = context.instance.layers;
    return written;
}

[shader("fragment")]
float4 rikka_entry_tint(RikkaDepthVaryings varyings) : SV_Target {
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    RikkaCoverage coverage;
    coverage.uv = varyings.uv;
    return float4(clamp(block->surface.transmittance(coverage, context), float3(0.0), float3(1.0)), 1.0);
}

[shader("fragment")]
RikkaEmissiveShade rikka_entry_forward_emissive(RikkaVaryings varyings, float4 frag_coord: SV_Position, bool front_facing: SV_IsFrontFace)
{
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    return rikka_forward_emissive<Model, Surface>(block->surface, block->model, context,
        rikka_pixel(context, varyings, frag_coord, front_facing, false));
}

[shader("fragment")]
RikkaEmissiveShade rikka_entry_forward_emissive_traced(RikkaVaryings varyings, float4 frag_coord: SV_Position, bool front_facing: SV_IsFrontFace)
{
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    RikkaPixel pixel = rikka_pixel(context, varyings, frag_coord, front_facing, false);
    pixel.traced = rikka_traced(context, pixel);
    return rikka_forward_emissive<Model, Surface>(block->surface, block->model, context, pixel);
}

struct RikkaScatterTargets {
    float4 diffuse : SV_Target0;
    float4 profile : SV_Target1;
};

[shader("fragment")]
RikkaScatterTargets rikka_entry_scatter(RikkaVaryings varyings, float4 frag_coord: SV_Position, bool front_facing: SV_IsFrontFace) {
    RikkaContext context = rikka_context(varyings.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    RikkaScatter scatter = rikka_scattered<Model, Surface>(block->surface, block->model, context, varyings, frag_coord, front_facing);
    RikkaScatterTargets targets;
    targets.diffuse = scatter.diffuse;
    targets.profile = scatter.profile;
    return targets;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void rikka_entry_hook(uint3 thread: SV_DispatchThreadID) {
    RikkaFrameRef frame = rikka_push.frame;
    RikkaHookJob job = ((RikkaHookJob*)rikka_address(frame.look.trace.hooks))[rikka_push.slot];
    uint vertex = thread.x + thread.y * rikka_push.flags;
    if (vertex >= job.vertex_count) {
        return;
    }
    RikkaContext context = rikka_context(job.instance);
    RikkaMaterialBlock* block = rikka_block<RikkaMaterialBlock>(context);
    RikkaVertex hooked =
        rikka_hooked_vertex<Surface>(block->surface, context, context.instance.vertex_offset + vertex);
    ((RikkaPositionRef)rikka_address(job.positions))[vertex] = hooked.world_position;
    ((RikkaAttributeRef)rikka_address(job.attributes))[vertex] = rikka_encode_attribute(hooked);
}
