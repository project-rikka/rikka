import plane;
import color;
import floating;

struct Push {
    Head head;
    float radius;
    uint blades;
    float rotation;
    float curvature;
    float brightness;
    float threshold;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint GATHER = 0u;
static const uint SPECULAR = 1u;
static const float TAU = 6.28318530718;

float shape_radius(float2 d) {
    if (pc.blades < 3u) {
        return pc.radius;
    }
    float sector = TAU / float(pc.blades);
    float phi = floored_mod(atan2(d.y, d.x) - pc.rotation, sector) - sector * 0.5;
    float polygon = pc.radius * cos(sector * 0.5) / cos(phi);
    return lerp(polygon, pc.radius, pc.curvature);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void lens(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    if (pc.head.mode == SPECULAR) {
        float4 c = pc.head.src.load(pixel);
        float l = c.a > 0.0 ? lum(c.rgb / c.a) : 0.0;
        float bright = smoothstep(pc.threshold - 0.02, pc.threshold + 0.02, l);
        c.rgb *= 1.0 + pc.brightness * bright;
        pc.head.dst.store(pixel, c);
        return;
    }
    int reach = int(ceil(pc.radius));
    float4 sum = float4(0.0);
    float total = 0.0;
    for (int y = -reach; y <= reach; y++) {
        for (int x = -reach; x <= reach; x++) {
            float2 d = float2(x, y);
            float w = clamp(shape_radius(d) - length(d) + 0.5, 0.0, 1.0);
            if (w <= 0.0) {
                continue;
            }
            sum += w * pc.head.src.load(pixel + int2(x, y));
            total += w;
        }
    }
    pc.head.dst.store(pixel, pc.head.clamped(sum / max(total, 1e-6)));
}
