import plane;
import color;

struct Push {
    Head head;
    Plane other;
    float4 k;
    float4 m;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint RATIO = 0u;
static const uint MULTIPLY = 1u;
static const uint SCREEN = 2u;
static const uint BRIGHT = 3u;
static const uint MIX = 4u;
static const uint FOCUS = 5u;
static const uint OVER = 6u;

float defocus(int2 pixel) {
    float2 size = float2(pc.head.dst.size());
    float2 p = (float2(pixel) + 0.5) / size.y;
    float2 center = float2(0.5 * size.x / size.y, pc.k[0]);
    float2 normal = float2(-pc.k[1], pc.k[2]);
    float distance = abs(dot(p - center, normal));
    return smoothstep(pc.k[3], pc.k[3] + max(pc.m[0], 1e-4), distance);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void arith(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 a = pc.head.src.load(pixel);
    float4 result;
    if (pc.head.mode == RATIO) {
        float4 b = pc.other.load(pixel);
        result = float4(a.rgb / max(b.rgb, float3(pc.k[0])), a.a);
    } else if (pc.head.mode == MULTIPLY) {
        float4 b = pc.other.load(pixel);
        result = float4(a.rgb * b.rgb, a.a);
    } else if (pc.head.mode == SCREEN) {
        float4 b = pc.other.load(pixel) * pc.k[0];
        result = float4(a.rgb + b.rgb - a.rgb * b.rgb / max(a.a, 1e-6), a.a);
    } else if (pc.head.mode == BRIGHT) {
        float3 straight = a.a > 0.0 ? a.rgb / a.a : float3(0.0);
        float l = lum(straight);
        float kept = smoothstep(pc.k[0], pc.k[0] + pc.k[1], l);
        result = float4(straight * kept * a.a, a.a);
    } else if (pc.head.mode == MIX) {
        float4 b = pc.other.load(pixel);
        result = lerp(a, b, pc.k[0]);
    } else if (pc.head.mode == OVER) {
        int2 at = pixel - int2(pc.k.xy);
        bool inside = all(at >= int2(0)) && all(at < pc.other.size());
        float4 b = inside ? pc.other.load(at) : float4(0.0);
        if (b.a <= 0.0) {
            result = a;
        } else {
            float alpha = b.a + a.a * (1.0 - b.a);
            float3 premultiplied = b.rgb * b.a + a.rgb * a.a * (1.0 - b.a);
            result = float4(premultiplied / alpha, alpha);
        }
    } else {
        float4 b = pc.other.load(pixel);
        float w = clamp(defocus(pixel) * 3.0 - pc.m[1], 0.0, 1.0);
        result = lerp(a, b, w);
    }
    pc.head.dst.store(pixel, pc.head.clamped(result));
}
