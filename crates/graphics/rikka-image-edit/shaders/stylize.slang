import plane;
import color;

struct Push {
    Head head;
    Plane other;
    float4 a;
    float4 b;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint EMBOSS = 0u;
static const uint EDGES = 1u;
static const uint LIGHTING = 2u;
static const uint VIGNETTE = 3u;

float4 emboss(int2 pixel) {
    float2 direction = pc.a.xy;
    float height = pc.a[2];
    float amount = pc.a[3];
    float2 at = float2(pixel) + 0.5;
    float4 center = pc.head.src.load(pixel);
    float3 ahead = pc.head.src.bilinear(at + direction * height).rgb;
    float3 behind = pc.head.src.bilinear(at - direction * height).rgb;
    float3 relief = 0.5 + (behind - ahead) * amount;
    float3 color = lerp(float3(lum(relief)), relief, pc.b[0]);
    return float4(color, center.a);
}

float4 edges(int2 pixel) {
    float3 gx = float3(0.0);
    float3 gy = float3(0.0);
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            float3 v = pc.head.src.load(pixel + int2(x, y)).rgb;
            float wx = float(x) * (y == 0 ? 2.0 : 1.0);
            float wy = float(y) * (x == 0 ? 2.0 : 1.0);
            gx += wx * v;
            gy += wy * v;
        }
    }
    float3 magnitude = sqrt(gx * gx + gy * gy) * 0.25;
    return float4(clamp(1.0 - magnitude, 0.0, 1.0), pc.head.src.load(pixel).a);
}

float4 lighting(int2 pixel) {
    float4 base = pc.head.src.load(pixel);
    float left = lum(pc.other.load(pixel - int2(1, 0)).rgb);
    float right = lum(pc.other.load(pixel + int2(1, 0)).rgb);
    float up = lum(pc.other.load(pixel - int2(0, 1)).rgb);
    float down = lum(pc.other.load(pixel + int2(0, 1)).rgb);
    float3 normal = normalize(float3((left - right) * pc.a[2], (up - down) * pc.a[2], 1.0));
    float3 light = normalize(float3(pc.a[0], pc.a[1], 1.0));
    float diffuse = dot(normal, light);
    float shine = pow(max(dot(reflect(-light, normal), float3(0.0, 0.0, 1.0)), 0.0), 16.0) * pc.a[3];
    float3 lit = base.rgb * (0.75 + 0.25 * diffuse / light.z) + shine;
    return float4(lit, base.a);
}

float4 vignette(int2 pixel) {
    float4 c = pc.head.src.load(pixel);
    float2 size = float2(pc.head.dst.size());
    float2 u = (float2(pixel) + 0.5 - size * 0.5) / (size * 0.5);
    float roundness = pc.a[2];
    float aspect = size.x / size.y;
    if (roundness > 0.0) {
        u.x *= lerp(1.0, aspect, roundness);
        u /= lerp(1.0, max(aspect, 1.0), roundness);
    }
    float power = roundness < 0.0 ? lerp(2.0, 8.0, -roundness) : 2.0;
    float d = pow(pow(abs(u.x), power) + pow(abs(u.y), power), 1.0 / power);
    float midpoint = pc.a[1];
    float feather = max(pc.a[3], 0.01);
    float f = smoothstep(midpoint, midpoint + feather, d);
    float amount = pc.a[0];
    float3 result;
    if (amount < 0.0) {
        float keep = 1.0 + amount * f;
        float highlights = pc.b[0] * smoothstep(0.6, 1.0, lum(c.rgb));
        result = c.rgb * lerp(keep, 1.0, highlights);
    } else {
        result = c.rgb + (1.0 - c.rgb) * amount * f;
    }
    return float4(result, c.a);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void stylize(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 result;
    switch (pc.head.mode) {
    case EMBOSS: result = emboss(pixel); break;
    case EDGES: result = edges(pixel); break;
    case LIGHTING: result = lighting(pixel); break;
    default: result = vignette(pixel); break;
    }
    pc.head.dst.store(pixel, pc.head.clamped(result));
}
