import rikka.depth;

[shader("vertex")]
RikkaDepthVaryings vertex_main(
    uint vertex_index: SV_VulkanVertexID,
    uint instance_index: SV_VulkanInstanceID,
    out float4 position: SV_Position)
{
    uint index = instance_index;
    RikkaInstance inst = rikka_push.frame.instances[index];

    float3 local = ((RikkaPositionRef)inst.positions)[vertex_index];
    float4 world = mul(inst.transform, float4(local, 1.0));

    RikkaDepthVaryings out;
    out.uv = RIKKA_CUTOUT
        ? rikka_decode_uv(((RikkaAttributeRef)inst.attributes)[vertex_index].uv)
        : float2(0.0);
    out.instance = index;
    out.world_position = world.xyz;

    float4x4 view_projection = (rikka_push.flags & RIKKA_PASS_SHADOW) != 0u
        ? rikka_push.frame.shadows[rikka_push.slot].view_projection
        : rikka_push.frame.view.view_projection;
    position = mul(view_projection, world);
    return out;
}

[shader("fragment")]
uint fragment_main(RikkaDepthVaryings varyings) : SV_Target {
    rikka_clip(rikka_push.frame, varyings.world_position);
    return rikka_identity(rikka_push.frame.instances[varyings.instance]);
}
