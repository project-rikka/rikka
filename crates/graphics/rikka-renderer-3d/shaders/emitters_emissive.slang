import rikka.emitter.draw;

struct EmissiveTargets {
    float4 color : SV_Target0;
    float4 emission : SV_Target1;
};

[shader("fragment")]
EmissiveTargets fragment_main(EmitterVaryings varyings, float4 frag_coord: SV_Position) {
    EmitterSample sample = rikka_emitter_fragment(varyings, frag_coord);
    if (rikka_emitter_faint(sample)) {
        discard;
    }
    EmissiveTargets targets;
    targets.color = sample.color;
    targets.emission = sample.emission;
    return targets;
}
