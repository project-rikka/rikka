import canvas.fx;
import canvas.color;

[shader("fragment")]
float4 fragment([[vk::location(0)]] float2 v_uv : TEXCOORD0) : SV_Target {
    float2 turned = float2(dot(fx.params[0].xy, v_uv), dot(fx.params[0].zw, v_uv))
        + fx.params[1].xy;
    float4 blended = fx_sample_level(fx_src_uv(turned), 0.0);
    if (blended.a <= 0.0) {
        return float4(0.0);
    }
    float3 light = max(blended.rgb, float3(0.0)) * (fx.params[1][2] / blended.a);
    float3 signal = pq_encode3(light);
    return float4(signal * blended.a, blended.a);
}
