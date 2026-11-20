import rikka.depth;
import rikka.temporal;

[shader("vertex")]
RikkaSurfaceVaryings vertex_main(
    uint vertex_index: SV_VulkanVertexID,
    uint instance_index: SV_VulkanInstanceID,
    out float4 position: SV_Position)
{
    uint index = instance_index;
    RikkaInstance inst = rikka_push.frame.instances[index];
    RikkaScene scene = rikka_push.frame.look.scene;

    float3 local = ((RikkaPositionRef)inst.positions)[vertex_index];
    float3 previous_local = local;
    if (scene.history != 0u && (inst.flags & RIKKA_DEFORMED) != 0u) {
        previous_local = rikka_push.frame.previous_positions[vertex_index];
    }
    RikkaAttribute packed = ((RikkaAttributeRef)inst.attributes)[vertex_index];

    float4 world = mul(inst.transform, float4(local, 1.0));
    float4 previous_world = mul(rikka_push.frame.previous_transforms[index], float4(previous_local, 1.0));

    RikkaSurfaceVaryings out;
    out.normal = mul(inst.normal_matrix, rikka_decode_octahedral(packed.normal));
    out.uv = rikka_decode_uv(packed.uv);
    out.clip = mul(rikka_push.frame.view.view_projection, world);
    position = out.clip;
    out.clip = rikka_unjittered(rikka_push.frame.view, out.clip);
    out.previous_clip = mul(scene.previous_view_projection, previous_world);
    out.instance = index;
    out.wet = rikka_wet_of(inst, vertex_index).a;
    return out;
}
