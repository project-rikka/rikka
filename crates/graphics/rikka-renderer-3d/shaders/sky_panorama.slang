import rikka.push;
import rikka.sky.panorama;

[shader("fragment")]
float4 fragment_main([[vk::location(0)]] float2 uv) : SV_Target {
    RikkaFrameRef frame = rikka_push.frame;
    float3 direction = rikka_view_ray(frame.view, uv * 2.0 - 1.0);
    float shown = frame.look.sky.panorama_background;
    return float4(rikka_panorama_background(frame, direction) * shown, shown);
}
