__exported import rikka.surface;

struct SpaceView {
    float4x4 view;
    float4x4 projection;
    float4x4 inverse_projection;
    float2 size;
    bool orthographic;
};

SpaceView space_view(RikkaFrameRef frame) {
    RikkaViewRef view = frame.view;
    SpaceView v;
    v.view = view.view;
    v.projection = view.projection;
    v.inverse_projection = view.inverse_projection;
    v.size = view.viewport;
    v.orthographic = view.projection[3][2] == 0.0;
    return v;
}

float3 space_position(SpaceView v, float2 uv, float depth) {
    float4 position = mul(v.inverse_projection, float4(uv * 2.0 - 1.0, depth, 1.0));
    return position.xyz / position.w;
}

float3 space_project(SpaceView v, float3 position) {
    float4 clip = mul(v.projection, float4(position, 1.0));
    float3 ndc = clip.xyz / clip.w;
    return float3(ndc.xy * 0.5 + 0.5, ndc.z);
}

float space_distance_of(float4x4 p, float depth) {
    float denominator = depth * p[3][2] - p[2][2];
    if (abs(denominator) < 1e-12) {
        return 1e30;
    }
    return -(p[2][3] - depth * p[3][3]) / denominator;
}

float space_distance(SpaceView v, float depth) {
    return space_distance_of(v.projection, depth);
}

float3 space_toward_eye(SpaceView v, float3 position) {
    return v.orthographic ? float3(0.0, 0.0, 1.0) : normalize(-position);
}

float2 space_uv(int2 pixel, float2 size) {
    return (float2(pixel) + 0.5) / size;
}

int2 space_pixel(float2 uv, float2 size) {
    return clamp(int2(uv * size), int2(0), int2(size) - 1);
}

bool space_on_screen(float2 uv) {
    return all(uv >= float2(0.0)) && all(uv < float2(1.0));
}

float2 space_encode_direction(float3 direction) {
    float3 n = direction / max(abs(direction.x) + abs(direction.y) + abs(direction.z), 1e-8);
    float2 e = n.xy;
    if (n.z < 0.0) {
        e = (1.0 - abs(e.yx)) * select(e >= float2(0.0), float2(1.0), float2(-1.0));
    }
    return e;
}

float3 space_decode_direction(float2 e) {
    float3 n = float3(e, 1.0 - abs(e.x) - abs(e.y));
    if (n.z < 0.0) {
        n.xy = (1.0 - abs(n.yx)) * select(n.xy >= float2(0.0), float2(1.0), float2(-1.0));
    }
    return normalize(n);
}

float space_random(inout uint state) {
    state = rikka_pcg_hash(state);
    return float(state >> 8u) * (1.0 / 16777216.0);
}
