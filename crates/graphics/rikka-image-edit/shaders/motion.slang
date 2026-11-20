import plane;

struct Push {
    Head head;
    float2 center;
    float amount;
    float angle;
    float density;
    float pad;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint LINEAR = 0u;
static const uint SPIN = 1u;
static const uint ZOOM = 2u;
static const int MOST = 1024;

[shader("compute")]
[numthreads(16, 16, 1)]
void motion(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float2 at = float2(pixel) + 0.5;
    float4 sum = float4(0.0);
    int count;
    if (pc.head.mode == LINEAR) {
        float2 along = float2(cos(pc.angle), -sin(pc.angle)) * pc.amount;
        count = clamp(int(ceil(pc.amount * pc.density)) + 1, 2, MOST);
        for (int i = 0; i < count; i++) {
            float t = float(i) / float(count - 1) - 0.5;
            sum += pc.head.src.bilinear(at + along * t);
        }
    } else if (pc.head.mode == SPIN) {
        float2 offset = at - pc.center;
        float reach = length(offset) * pc.amount;
        count = clamp(int(ceil(reach * pc.density)) + 1, 2, MOST);
        for (int i = 0; i < count; i++) {
            float turn = (float(i) / float(count - 1) - 0.5) * pc.amount;
            float c = cos(turn);
            float s = sin(turn);
            sum += pc.head.src.bilinear(pc.center + float2(c * offset.x - s * offset.y, s * offset.x + c * offset.y));
        }
    } else {
        float2 offset = at - pc.center;
        float reach = length(offset) * pc.amount;
        count = clamp(int(ceil(reach * pc.density)) + 1, 2, MOST);
        for (int i = 0; i < count; i++) {
            float t = 1.0 - pc.amount * float(i) / float(count - 1);
            sum += pc.head.src.bilinear(pc.center + offset * t);
        }
    }
    pc.head.dst.store(pixel, pc.head.clamped(sum / float(count)));
}
