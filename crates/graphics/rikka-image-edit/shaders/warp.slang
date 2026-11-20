import plane;

struct Push {
    Head head;
    float4 row0;
    float4 row1;
    float4 row2;
    float4 fill;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint NEAREST = 0u;
static const uint BILINEAR = 1u;
static const uint BICUBIC = 2u;
static const int MAX_SAMPLES = 8;

float2 mapped(float2 p) {
    float3 h = float3(dot(pc.row0.xyz, float3(p, 1.0)), dot(pc.row1.xyz, float3(p, 1.0)), dot(pc.row2.xyz, float3(p, 1.0)));
    return h.xy / h.z;
}

float4 sample_at(float2 at, float2 reach) {
    float2 size = float2(pc.head.src.size());
    if (pc.head.mode == NEAREST) {
        bool inside = all(at >= float2(0.0)) && all(at < size);
        return inside ? pc.head.src.load(int2(floor(at))) : pc.fill;
    }
    float2 low = max(at - reach, float2(0.0));
    float2 high = min(at + reach, size);
    float2 inside = clamp((high - low) / (2.0 * reach), 0.0, 1.0);
    float coverage = inside.x * inside.y;
    if (coverage <= 0.0) {
        return pc.fill;
    }
    float4 value = pc.head.mode == BILINEAR ? pc.head.src.bilinear(at) : pc.head.src.bicubic(at);
    return lerp(pc.fill, value, coverage);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void warp(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float2 p = float2(pixel) + 0.5;
    float2 center = mapped(p);
    float2 dx = mapped(p + float2(1.0, 0.0)) - center;
    float2 dy = mapped(p + float2(0.0, 1.0)) - center;
    int n = pc.head.mode == NEAREST ? 1 : clamp(int(ceil(max(length(dx), length(dy)) - 1e-3)), 1, MAX_SAMPLES);
    float2 reach = max(0.5 * (abs(dx) + abs(dy)) / float(n), float2(1e-3));
    float4 sum = float4(0.0);
    for (int y = 0; y < n; y++) {
        for (int x = 0; x < n; x++) {
            float2 offset = (float2(x, y) + 0.5) / float(n) - 0.5;
            sum += sample_at(mapped(p + offset), reach);
        }
    }
    pc.head.dst.store(pixel, pc.head.clamped(sum / float(n * n)));
}
