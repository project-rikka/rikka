import fluid;

struct Push {
    ReadUints keys;
    ReadUints slots;
    ReadUints starts;
    Uints order;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint p = thread.x;
    if (p >= pc.count) {
        return;
    }
    uint key = pc.keys[p];
    pc.order[pc.starts[key] + pc.slots[p]] = p;
}
