import fluid;

struct Push {
    ReadVec4s drape;
    Vec4s skinned;
    uint first;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint v = thread.x;
    if (v >= pc.count) {
        return;
    }
    float4 placed = pc.drape[3u * v];
    float blend = clamp(placed.w, 0.0, 1.0);
    if (!(blend > 0.0)) {
        return;
    }
    float4 skinned = pc.skinned[pc.first + v];
    pc.skinned[pc.first + v] = float4(lerp(skinned.xyz, placed.xyz, blend), skinned.w);
}
