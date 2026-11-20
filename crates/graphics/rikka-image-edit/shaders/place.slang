import plane;
import color;

struct Push {
    Head head;
    Plane base;
    float* params;
    float* tables;
    uint transfer;
    uint pad;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint DISSOLVE = 26u;
static const uint REPLACE = 64u;
static const uint NEAREST = 0u;
static const uint BILINEAR = 1u;
static const int MAX_SAMPLES = 4;

float p(uint index) {
    return pc.params[index];
}

bool mirrored() {
    return p(16u) != 0.0;
}

float3 projected(float2 q) {
    float3 h = float3(q, 1.0);
    return float3(
        dot(float3(p(0u), p(1u), p(2u)), h),
        dot(float3(p(3u), p(4u), p(5u)), h),
        dot(float3(p(6u), p(7u), p(8u)), h));
}

float2 mapped(float2 q) {
    float3 h = projected(q);
    return h.xy / h.z;
}

float4 sample_source(float2 at, uint filter_kind) {
    if (mirrored()) {
        at = mirror_wrap(at, float2(p(9u), p(10u)));
    }
    float2 reduced = at * float2(p(11u), p(12u));
    if (filter_kind == NEAREST) {
        return pc.head.src.load(int2(floor(reduced)));
    }
    if (filter_kind == BILINEAR) {
        return pc.head.src.bilinear(reduced);
    }
    return max(pc.head.src.bicubic(reduced), float4(0.0));
}

float edge(float2 s, float2 dx, float2 dy) {
    if (mirrored()) {
        return 1.0;
    }
    float2 size = float2(p(9u), p(10u));
    float radius = p(13u);
    float2 half_size = size * 0.5;
    float2 centered = s - half_size;
    float2 q = abs(centered) - (half_size - radius);
    float distance = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius;
    float2 normal;
    if (q.x > 0.0 && q.y > 0.0) {
        normal = normalize(q);
    } else if (q.x > q.y) {
        normal = float2(1.0, 0.0);
    } else {
        normal = float2(0.0, 1.0);
    }
    normal *= float2(centered.x < 0.0 ? -1.0 : 1.0, centered.y < 0.0 ? -1.0 : 1.0);
    float stretch = length(float2(dot(normal, dx), dot(normal, dy)));
    return clamp(0.5 - distance / max(stretch, 1e-6), 0.0, 1.0);
}

float4 composed(float4 base, float3 top, float alpha, int2 pixel) {
    if (pc.head.mode == REPLACE) {
        return float4(top, alpha);
    }
    if (pc.head.mode == DISSOLVE) {
        bool kept = hash01(uint3(uint2(pixel), 0x9e37u)) < alpha;
        return kept ? float4(top, 1.0) : base;
    }
    float3 mixed = blended(pc.head.mode, base.rgb, top);
    float under = base.a * (1.0 - alpha);
    float both = alpha * base.a;
    float3 color = alpha * (1.0 - base.a) * top + under * base.rgb + both * mixed;
    float total = alpha + under;
    return float4(total > 0.0 ? color / total : float3(0.0), total);
}

float4 untouched(float4 base) {
    return pc.head.mode == REPLACE ? float4(0.0) : base;
}

[shader("compute")]
[numthreads(16, 16, 1)]
void place(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 base = pc.base.load(pixel);
    float2 center = float2(pixel) + 0.5;
    float3 h = projected(center);
    if (h.z <= 0.0) {
        pc.head.dst.store(pixel, pc.head.clamped(untouched(base)));
        return;
    }
    float2 s = h.xy / h.z;
    float2 dx = mapped(center + float2(1.0, 0.0)) - s;
    float2 dy = mapped(center + float2(0.0, 1.0)) - s;
    float coverage = edge(s, dx, dy) * p(14u);
    if (coverage <= 0.0) {
        pc.head.dst.store(pixel, pc.head.clamped(untouched(base)));
        return;
    }
    uint filter_kind = uint(p(15u));
    float2 ratio = float2(p(11u), p(12u));
    float footprint = max(length(dx * ratio), length(dy * ratio));
    int n = filter_kind == NEAREST ? 1 : clamp(int(ceil(footprint - 1e-3)), 1, MAX_SAMPLES);
    float4 sum = float4(0.0);
    for (int y = 0; y < n; y++) {
        for (int x = 0; x < n; x++) {
            float2 offset = (float2(x, y) + 0.5) / float(n) - 0.5;
            sum += sample_source(mapped(center + offset), filter_kind);
        }
    }
    float4 color = sum / float(n * n);
    float alpha = clamp(color.a, 0.0, 1.0) * coverage;
    if (alpha <= 0.0) {
        pc.head.dst.store(pixel, pc.head.clamped(untouched(base)));
        return;
    }
    float3 straight = color.a > 0.0 ? max(color.rgb / color.a, float3(0.0)) : float3(0.0);
    float3 top = from_linear(straight, pc.transfer, pc.tables);
    pc.head.dst.store(pixel, pc.head.clamped(composed(base, top, alpha, pixel)));
}
