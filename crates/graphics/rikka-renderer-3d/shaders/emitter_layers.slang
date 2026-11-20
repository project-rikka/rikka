import rikka.emitter.draw;

static const float LAYER_COVERAGE = 0.5;

[shader("fragment")]
uint fragment_main(EmitterVaryings varyings, float4 frag_coord: SV_Position) : SV_Target {
    EmitterSample sample = rikka_emitter_fragment(varyings, frag_coord);
    if (sample.weight < LAYER_COVERAGE) {
        discard;
    }
    return rikka_pushed_emitter().layers;
}
