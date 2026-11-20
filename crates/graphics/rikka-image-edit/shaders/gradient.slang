import plane;

struct Push {
    Head head;
    float2 from;
    float2 to;
    float4 start;
    float4 end;
    float dither;
    uint ink;
    uint pad0;
    uint pad1;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint LINEAR = 0u;
static const uint REFLECTED = 1u;
static const uint RADIAL = 2u;
static const uint DIAMOND = 3u;
static const uint CONICAL = 4u;
static const uint PAINTS = 0u;
static const uint FADES = 1u;
static const float PI = 3.14159265358979;

float position(float2 p) {
    float2 d = pc.to - pc.from;
    float span = dot(d, d);
    if (span <= 1e-12) {
        return 0.0;
    }
    float2 q = p - pc.from;
    float along = dot(q, d) / span;
    float across = (q.y * d.x - q.x * d.y) / span;
    if (pc.head.mode == REFLECTED) {
        return abs(along);
    }
    if (pc.head.mode == RADIAL) {
        return sqrt(dot(q, q) / span);
    }
    if (pc.head.mode == DIAMOND) {
        return abs(along) + abs(across);
    }
    if (pc.head.mode == CONICAL) {
        if (dot(q, q) <= 1e-12) {
            return 0.0;
        }
        return abs(atan2(across, along)) / PI;
    }
    return along;
}

float noise(int2 pixel) {
    uint2 at = uint2(pixel);
    return hash01(uint3(at, 0x9e3779b9u)) - hash01(uint3(at, 0x7f4a7c15u));
}

[shader("compute")]
[numthreads(16, 16, 1)]
void gradient(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 c = pc.head.src.load(pixel);
    float raw = position(float2(pixel) + 0.5);
    float t = clamp(raw, 0.0, 1.0);
    float grain = raw > 0.0 && raw < 1.0 ? noise(pixel) * pc.dither : 0.0;
    if (pc.ink == FADES) {
        float kept = lerp(pc.start[3], pc.end[3], t);
        pc.head.dst.store(pixel, pc.head.clamped(float4(c.rgb, c.a * kept + grain)));
        return;
    }
    float4 start = float4(pc.start.rgb * pc.start[3], pc.start[3]);
    float4 end = float4(pc.end.rgb * pc.end[3], pc.end[3]);
    float4 top = lerp(start, end, t);
    float4 under = float4(c.rgb * c.a, c.a);
    float4 laid = top + under * (1.0 - top.a);
    float3 color = laid.a > 0.0 ? laid.rgb / laid.a : float3(0.0);
    pc.head.dst.store(pixel, pc.head.clamped(float4(color + grain, laid.a)));
}
