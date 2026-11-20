import plane;

struct Push {
    Head head;
    float radius;
    float sharpness;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const int SECTORS = 8;
static const float TAU = 6.28318530718;

[shader("compute")]
[numthreads(16, 16, 1)]
void kuwahara(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    int reach = int(ceil(pc.radius));
    float sigma = max(pc.radius * 0.5, 0.5);
    float4 sums[SECTORS];
    float3 squares[SECTORS];
    float weights[SECTORS];
    for (int k = 0; k < SECTORS; k++) {
        sums[k] = float4(0.0);
        squares[k] = float3(0.0);
        weights[k] = 0.0;
    }
    for (int y = -reach; y <= reach; y++) {
        for (int x = -reach; x <= reach; x++) {
            float2 d = float2(x, y);
            float r = length(d);
            if (r > pc.radius) {
                continue;
            }
            float radial = exp(-r * r / (2.0 * sigma * sigma));
            float4 v = pc.head.src.load(pixel + int2(x, y));
            float3 straight = v.a > 0.0 ? v.rgb / v.a : float3(0.0);
            float angle = r > 0.0 ? atan2(d.y, d.x) : 0.0;
            for (int k = 0; k < SECTORS; k++) {
                float offset = angle - TAU * float(k) / float(SECTORS);
                float c = cos(offset);
                float sector = r > 0.0 ? max(c, 0.0) : 1.0;
                float w = radial * sector * sector;
                if (w <= 0.0) {
                    continue;
                }
                sums[k] += w * v;
                squares[k] += w * straight * straight;
                weights[k] += w;
            }
        }
    }
    float4 result = float4(0.0);
    float total = 0.0;
    for (int k = 0; k < SECTORS; k++) {
        if (weights[k] <= 0.0) {
            continue;
        }
        float4 mean = sums[k] / weights[k];
        float3 straight_mean = mean.a > 0.0 ? mean.rgb / mean.a : float3(0.0);
        float3 variance = abs(squares[k] / weights[k] - straight_mean * straight_mean);
        float spread = sqrt(variance.r + variance.g + variance.b) * 255.0;
        float alpha = 1.0 / (1.0 + pow(spread, pc.sharpness));
        result += alpha * mean;
        total += alpha;
    }
    pc.head.dst.store(pixel, pc.head.clamped(result / max(total, 1e-9)));
}
