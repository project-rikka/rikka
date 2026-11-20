import rikka.push;
import rikka.fluid;
import rikka.fluid.splat;

struct Amounts {
    float4 thickness: SV_Target0;
    float4 scattering: SV_Target1;
};

static const int2 TAPS[4] = { int2(0, 0), int2(1, 0), int2(0, 1), int2(1, 1) };

[shader("fragment")]
Amounts fragment_main(RikkaFluidSplat splat, float4 frag_coord: SV_Position) {
    float2 span;
    if (!rikka_fluid_hit(splat.origin, splat.direction, span)) {
        discard;
    }
    float enter = max(span.x, 0.0);
    float leave = span.y;
    RikkaFrameRef frame = rikka_push.frame;
    uint image = frame.fluid.scene_depth;
    int2 full = int2(frag_coord.xy) * 2;
    int2 limit = int2(frame.view.viewport) - 1;
    float scene = 1.0;
    [unroll]
    for (int i = 0; i < 4; ++i) {
        scene = min(scene, rikka_fetch(rikka_images[image], min(full + TAPS[i], limit), 0).r);
    }
    scene = rikka_fluid_biased_depth(frame.view.projection, frame.fluid.radius, scene);
    if (scene > 0.0) {
        float denominator = (splat.clip.z - splat.clip.x) - scene * (splat.clip.w - splat.clip.y);
        if (abs(denominator) > 1e-20) {
            leave = min(leave, (scene * splat.clip.y - splat.clip.x) / denominator);
        }
    }
    float chord = max(leave - enter, 0.0) * length(splat.span) * splat.volume;
    if (chord <= 0.0) {
        discard;
    }
    Amounts amounts;
    amounts.thickness = float4(splat.absorption.rgb * chord, chord);
    amounts.scattering = splat.scattering * chord;
    return amounts;
}
