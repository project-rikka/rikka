import canvas.fx;

[shader("fragment")]
float4 fragment([[vk::location(0)]] float2 v_uv : TEXCOORD0) : SV_Target {
    float2 turned = float2(dot(fx.params[0].xy, v_uv), dot(fx.params[0].zw, v_uv))
        + fx.params[1].xy;
    return fx_sample_level(fx_src_uv(turned), 0.0);
}
