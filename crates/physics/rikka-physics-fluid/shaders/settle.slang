import fluid;

struct Push {
    ReadFloats vorticity_magnitude;
    Vec4s positions;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    pc.positions.at(i)->w = pc.vorticity_magnitude[i];
}
