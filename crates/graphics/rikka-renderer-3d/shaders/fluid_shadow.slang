import rikka.fluid;
import rikka.fluid.splat;

static const float CASTING = 0.5;

[shader("fragment")]
void fragment_main(RikkaFluidSplat splat, float4 frag_coord: SV_Position, out float depth: SV_DepthLessEqual) {
    float2 span;
    if (!rikka_fluid_hit(splat.origin, splat.direction, span)) {
        discard;
    }
    float enter = max(span.x, 0.0);
    float chord = max(span.y - enter, 0.0) * length(splat.span) * splat.volume;
    if (1.0 - exp(-splat.absorption.a * chord) < CASTING) {
        discard;
    }
    float inside = enter + 0.25 * (span.y - enter);
    depth = min(rikka_fluid_depth_at(splat.clip, inside), frag_coord.z);
}
