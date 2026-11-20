import rikka.fluid.composite;

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    return rikka_fluid_composite(frag_coord, true);
}
