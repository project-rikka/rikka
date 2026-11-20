import cloth.common;

struct Push {
    ReadVec4s rest;
    ReadUvec4s joints;
    ReadVec4s weights;
    Mat4s palettes;
    Vec4s skinned;
    uint vertices;
    uint boundaries;
    uint slots;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint at = thread.x;
    if (at >= pc.vertices * pc.boundaries) {
        return;
    }
    uint boundary = at / pc.vertices;
    uint vertex = at - boundary * pc.vertices;
    uint4 joint = pc.joints[vertex];
    float4 weight = pc.weights[vertex];
    uint base = boundary * pc.slots;
    float4x4 blended = scaled(pc.palettes[base + joint.x].columns, weight.x);
    blended += scaled(pc.palettes[base + joint.y].columns, weight.y);
    blended += scaled(pc.palettes[base + joint.z].columns, weight.z);
    blended += scaled(pc.palettes[base + joint.w].columns, weight.w);
    pc.skinned[at] = float4(mul(float4(pc.rest[vertex].xyz, 1.0), blended).xyz, 0.0);
}
