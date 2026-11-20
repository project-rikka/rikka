import fluid;
import physics.matrix;

struct Push {
    ReadVec4s rest;
    ReadUvec4s joints;
    ReadUvec2s weights;
    Mat4s palette;
    ReadUints morph_starts;
    ReadVec4s morph_deltas;
    ReadFloats morph_weights;
    Vec4s skinned;
    uint count;
    uint targets;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint v = thread.x;
    if (v >= pc.count) {
        return;
    }
    float3 position = pc.rest[v].xyz;
    if (pc.targets > 0u) {
        uint last = pc.morph_starts[v + 1u];
        for (uint at = pc.morph_starts[v]; at < last; ++at) {
            float4 delta = pc.morph_deltas[at];
            position += delta.xyz * pc.morph_weights[uint(delta.w)];
        }
    }
    uint4 joint = pc.joints[v];
    uint2 packed = pc.weights[v];
    float weight[4] = {
        float(packed.x & 0xffffu) / 65535.0,
        float(packed.x >> 16) / 65535.0,
        float(packed.y & 0xffffu) / 65535.0,
        float(packed.y >> 16) / 65535.0
    };
    float4x4 blended = float4x4(0.0);
    for (int k = 0; k < 4; ++k) {
        if (weight[k] > 0.0) {
            blended += scaled(pc.palette[joint[k]].columns, weight[k]);
        }
    }
    pc.skinned[v] = float4(mul(float4(position, 1.0), blended).xyz, 0.0);
}
