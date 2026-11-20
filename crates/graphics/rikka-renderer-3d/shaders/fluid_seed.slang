import rikka.push;
import rikka.fluid;

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position, out float depth: SV_Depth) : SV_Target {
    RikkaFrameRef frame = rikka_push.frame;
    float scene = rikka_fetch(rikka_images[frame.fluid.scene_depth], int2(frag_coord.xy), 0).r;
    depth = rikka_fluid_biased_depth(frame.view.projection, frame.fluid.radius, scene);
    return float4(0.0);
}
