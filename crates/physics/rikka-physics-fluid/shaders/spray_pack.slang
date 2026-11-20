import fluid;

struct Push {
    ReadVec4s foam_positions;
    ReadVec4s foam_velocities;
    ReadUints foam_meta;
    ReadUints counters;
    Vec4s spray;
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
    if (i >= pc.counters[FOAM_COUNT]) {
        pc.spray[2u * i] = float4(0.0);
        pc.spray[2u * i + 1u] = float4(0.0);
        return;
    }
    float4 x = pc.foam_positions[i];
    pc.spray[2u * i] = float4(x.xyz, clamp(x.w, 0.0, 1.0));
    pc.spray[2u * i + 1u] = float4(pc.foam_velocities[i].xyz, asfloat(pc.foam_meta[i]));
}
