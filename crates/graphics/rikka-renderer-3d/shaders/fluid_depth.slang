import rikka.fluid;
import rikka.fluid.splat;

[shader("fragment")]
float4 fragment_main(
    RikkaFluidSplat splat,
    float4 frag_coord: SV_Position,
    out float depth: SV_DepthLessEqual)
    : SV_Target
{
    float2 span;
    if (!rikka_fluid_hit(splat.origin, splat.direction, span)) {
        discard;
    }
    depth = min(rikka_fluid_depth_at(splat.clip, max(span.x, 0.0)), frag_coord.z);
    return splat.front;
}
