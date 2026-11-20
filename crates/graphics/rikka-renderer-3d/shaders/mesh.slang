import rikka.push;
import rikka.fragment;

[shader("vertex")]
RikkaVaryings vertex_main(
    uint vertex_index: SV_VulkanVertexID,
    uint instance_index: SV_VulkanInstanceID,
    out float4 position: SV_Position)
{
    uint index = instance_index;
    RikkaInstance inst = rikka_push.frame.instances[index];

    float3 local = ((RikkaPositionRef)inst.positions)[vertex_index];
    RikkaAttribute packed = ((RikkaAttributeRef)inst.attributes)[vertex_index];

    float4 world = mul(inst.transform, float4(local, 1.0));
    float4 tangent = rikka_decode_tangent(packed.tangent);

    RikkaVaryings out;
    out.world_position = world.xyz;
    out.normal = mul(inst.normal_matrix, rikka_decode_octahedral(packed.normal));

    out.tangent = float4(mul((float3x3)inst.transform, tangent.xyz), tangent.w);
    out.uv = rikka_decode_uv(packed.uv);
    out.color = rikka_decode_color(packed.color);
    out.instance = index;
    out.wet = rikka_wet_of(inst, vertex_index);

    position = mul(rikka_push.frame.view.view_projection, world);
    return out;
}
