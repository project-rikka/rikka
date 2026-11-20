import plane;

struct Push {
    Head head;
    Plane other;
    float4 k;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint DISPLACE = 0u;
static const uint LENS = 1u;

float4 sampled(float2 at) {
    return pc.head.src.bilinear(mirror_wrap(at, float2(pc.head.src.size())));
}

float2 lens(float2 at) {
    float2 size = float2(pc.head.src.size());
    float2 offset = at - pc.k.xy;
    float reach = max(length(size * 0.5), 1.0);
    float r2 = dot(offset, offset) / (reach * reach);
    return pc.k.xy + offset * (1.0 + pc.k[2] * r2);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void distort(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float2 at = float2(pixel) + 0.5;
    float4 result;
    if (pc.head.mode == DISPLACE) {
        float2 size = float2(pc.head.src.size());
        float2 map_size = float2(pc.other.size());
        float4 shift = pc.other.bilinear(at / size * map_size);
        float2 offset = (clamp(shift.rg, 0.0, 1.0) - 0.5) * 2.0 * pc.k.xy;
        result = sampled(at + offset);
    } else {
        float4 sum = float4(0.0);
        for (int y = 0; y < 2; y++) {
            for (int x = 0; x < 2; x++) {
                float2 offset = (float2(x, y) + 0.5) / 2.0 - 0.5;
                sum += sampled(lens(at + offset));
            }
        }
        result = sum / 4.0;
    }
    pc.head.dst.store(pixel, pc.head.clamped(result));
}
