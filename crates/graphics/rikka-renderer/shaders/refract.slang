import canvas.fx;
import canvas.lens;

[shader("fragment")]
float4 fragment([[vk::location(0)]] float2 v_uv : TEXCOORD0) : SV_Target {
    return lens_through(fx_src_uv(v_uv));
}
