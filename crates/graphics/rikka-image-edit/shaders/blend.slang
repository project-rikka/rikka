import plane;
import color;

struct Push {
    Head head;
    Plane top;
    Plane mask;
    float opacity;
    uint masked;
    uint seed;
    uint pad;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint DISSOLVE = 26u;

[shader("compute")]
[numthreads(16, 16, 1)]
void blend(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 base = pc.head.src.load(pixel);
    float4 top = pc.top.load(pixel);
    float amount = pc.opacity;
    if (pc.masked != 0u) {
        amount *= clamp(pc.mask.load(pixel).r, 0.0, 1.0);
    }
    float4 result;
    if (pc.head.mode == DISSOLVE) {
        bool kept = hash01(uint3(uint2(pixel + pc.head.window), pc.seed)) < amount * top.a;
        result = kept ? float4(top.rgb, 1.0) : base;
        result.a = kept ? max(base.a, top.a > 0.0 ? 1.0 : 0.0) : base.a;
    } else {
        float3 mixed = blended(pc.head.mode, base.rgb, top.rgb);
        float4 under = float4(base.rgb * base.a, base.a);
        float4 over = float4(mixed * top.a, top.a);
        float4 premultiplied = lerp(under, over, amount);
        result = float4(premultiplied.a > 0.0 ? premultiplied.rgb / premultiplied.a : float3(0.0), premultiplied.a);
    }
    pc.head.dst.store(pixel, pc.head.clamped(result));
}
