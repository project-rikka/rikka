import fluid;

struct Push {
    ReadUints record;
    Vec4s wet;
    uint first;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint k = thread.x;
    if (k >= pc.count) {
        return;
    }
    pc.wet[k] = unpackUnorm4x8ToFloat(pc.record[pc.first + k]);
}
