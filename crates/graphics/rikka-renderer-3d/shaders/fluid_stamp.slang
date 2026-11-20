import rikka.push;
import rikka.fluid.strand;

[shader("fragment")]
uint4 fragment_main(float4 frag_coord: SV_Position, out float frag_depth: SV_Depth) : SV_Target {
    frag_depth = 0.0;
    int2 pixel = int2(frag_coord.xy);
    RikkaFluidRef fluid = rikka_push.frame.fluid;
    float distance = rikka_fetch(rikka_images[fluid.smoothed], pixel, 0).r;
    if (distance <= 0.0) {
        discard;
    }
    RikkaViewRef view = rikka_push.frame.view;
    float2 uv = (float2(pixel) + 0.5) * view.inverse_viewport;
    float path = rikka_fluid_amount(view, fluid, fluid.thickness, uv).a;
    if (rikka_fluid_coverage(view, fluid, uv, distance, path) < 0.5) {
        discard;
    }
    float3 position = rikka_fluid_position(view, uv, distance);
    float3 eye = rikka_fluid_eye(view, position);
    float thin = 1.0 - rikka_fluid_outline(view, fluid, uv, distance).y;
    RikkaFluidStrand strand = rikka_fluid_strand(rikka_push.frame, pixel, uv, position, distance, eye, thin);
    float clouded = 1.0 - exp(-rikka_fluid_amount(view, fluid, fluid.scattering, uv).a);
    float3 normal = rikka_fluid_surface_normal(strand,
        rikka_fluid_shading_normal(view, fluid, pixel, position, distance, eye, clouded), eye);
    float4 front = rikka_fluid_front(view, fluid, pixel);
    float ior = max(front.a, 1.0001);
    float reflection = (ior - 1.0) / (ior + 1.0);

    frag_depth = rikka_fluid_depth_of(view.projection, distance);
    return uint4(
        rikka_encode_octahedral(normal),
        packHalf2x16(front.xy),
        0u,
        packUnorm4x8(float4(clamp(front.b, 0.0, 1.0), reflection, 0.0, 0.0)));
}
