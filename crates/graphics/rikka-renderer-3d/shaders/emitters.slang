import rikka.emitter.draw;

[shader("vertex")]
EmitterVaryings vertex_main(
    uint vertex_index: SV_VulkanVertexID,
    uint instance_index: SV_VulkanInstanceID,
    out float4 position: SV_Position)
{
    return rikka_emitter_vertex(vertex_index, instance_index, position);
}

[shader("fragment")]
float4 fragment_main(EmitterVaryings varyings, float4 frag_coord: SV_Position) : SV_Target {
    EmitterSample sample = rikka_emitter_fragment(varyings, frag_coord);
    if (rikka_emitter_faint(sample)) {
        discard;
    }
    return sample.color;
}
