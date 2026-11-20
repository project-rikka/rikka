import plane;

struct Push {
    Head head;
    float4 area;
    float invert;
    float pad0;
    float pad1;
    float pad2;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint RECTANGLE = 0u;
static const uint ELLIPSE = 1u;
static const uint COPY = 2u;
static const uint LUMA = 3u;
static const uint ALPHA = 4u;

float coverage(float2 p) {
    float2 low = pc.area.xy;
    float2 high = pc.area.xy + pc.area.zw;
    if (pc.head.mode == RECTANGLE) {
        float2 inside = clamp(min(p + 0.5, high) - max(p - 0.5, low), 0.0, 1.0);
        return inside.x * inside.y;
    }
    float2 center = (low + high) * 0.5;
    float2 radii = max(pc.area.zw * 0.5, float2(1e-3));
    float2 q = (p - center) / radii;
    float k = length(q);
    float gradient = length(q / radii);
    float distance = (k - 1.0) * k / max(gradient, 1e-6);
    return clamp(0.5 - distance, 0.0, 1.0);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void mask(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float value;
    if (pc.head.mode == COPY) {
        value = pc.head.src.load(pixel).r;
    } else if (pc.head.mode == LUMA) {
        float4 c = pc.head.src.load(pixel);
        value = dot(clamp(c.rgb, 0.0, 1.0), float3(0.2126, 0.7152, 0.0722)) * clamp(c.a, 0.0, 1.0);
    } else if (pc.head.mode == ALPHA) {
        value = pc.head.src.load(pixel).a;
    } else {
        value = coverage(float2(pixel + pc.head.window) + 0.5);
    }
    value = lerp(value, 1.0 - value, pc.invert);
    pc.head.dst.store(pixel, float4(clamp(value, 0.0, 1.0), 0.0, 0.0, 1.0));
}
