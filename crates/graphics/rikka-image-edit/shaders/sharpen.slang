import plane;
import color;

struct Push {
    Head head;
    Plane blurred;
    float amount;
    float threshold;
    float pad0;
    float pad1;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint UNSHARP = 0u;
static const uint HIGH_PASS = 1u;
static const uint CLARITY = 2u;

[shader("compute")]
[numthreads(16, 16, 1)]
void sharpen(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 original = pc.head.src.load(pixel);
    float4 blurred = pc.blurred.load(pixel);
    float3 soft = blurred.a > 0.0 ? blurred.rgb / blurred.a : original.rgb;
    float3 detail = original.rgb - soft;
    float3 result;
    if (pc.head.mode == UNSHARP) {
        float3 kept = step(float3(pc.threshold), abs(detail));
        result = original.rgb + pc.amount * detail * kept;
    } else if (pc.head.mode == HIGH_PASS) {
        result = detail + 0.5;
    } else {
        float y = lum(original.rgb);
        float midtones = 4.0 * y * (1.0 - y);
        float lift = lum(detail) * pc.amount * midtones;
        result = original.rgb + lift;
    }
    pc.head.dst.store(pixel, pc.head.clamped(float4(result, original.a)));
}
