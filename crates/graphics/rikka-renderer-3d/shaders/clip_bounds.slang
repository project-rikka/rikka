import rikka.common;

struct Push {
    RikkaFrameRef frame;
    uint pad[2];
    float4 planes[6];
    float4 color;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float PARALLEL = 1e-8;

float3 unprojected(RikkaViewRef view, float2 ndc, float depth) {
    float4 point = mul(view.inverse_view_projection, float4(ndc, depth, 1.0));
    return point.xyz / point.w;
}

[shader("fragment")]
float4 fragment_main([[vk::location(0)]] float2 uv) : SV_Target {
    RikkaViewRef view = push.frame.view;
    float2 ndc = uv * 2.0 - 1.0;
    float3 near_point = unprojected(view, ndc, 1.0);
    bool perspective = view.projection[3][3] < 0.5;
    float3 origin = perspective ? view.camera_position : near_point;
    float3 direction = unprojected(view, ndc, 0.5) - near_point;
    float enter = perspective ? 0.0 : -1e30;
    float leave = 1e30;
    for (uint face = 0u; face < 6u; face++) {
        float4 plane = push.planes[face];
        float toward = dot(plane.xyz, direction);
        float distance = dot(plane.xyz, origin) + plane.w;
        if (abs(toward) < PARALLEL) {
            if (distance < 0.0) {
                discard;
            }
            continue;
        }
        float t = -distance / toward;
        if (toward > 0.0) {
            enter = max(enter, t);
        } else {
            leave = min(leave, t);
        }
    }
    if (!(enter <= leave)) {
        discard;
    }
    if (perspective) {
        bool inside = true;
        for (uint face = 0u; face < 6u; face++) {
            float4 plane = push.planes[face];
            inside = inside && dot(plane.xyz, origin) + plane.w >= 0.0;
        }
        if (inside) {
            discard;
        }
    }
    return float4(push.color.rgb, 1.0);
}
