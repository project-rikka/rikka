import rikka.emitter.draw;

[shader("fragment")]
float4 fragment_main(EmitterVaryings varyings, float4 frag_coord: SV_Position) : SV_Target {
    EmitterSample sample = rikka_emitter_fragment(varyings, frag_coord);
    if (rikka_emitter_faint(sample)) {
        discard;
    }
    float weight = saturate(sample.weight);
    return float4(rikka_emitter_motion(varyings) * weight, weight, 0.0);
}
