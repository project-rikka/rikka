import fluid;

struct Push {
    ReadVec4s spawns;
    Vec4s positions;
    Vec4s velocities;
    Uints phases;
    Vec4s warm;
    Uints bond_counts;
    Uvec4s anchors;
    ReadVec4s skin_from;
    ReadVec4s skin_to;
    ReadUvec4s triangles;
    uint first;
    uint count;
    uint base;
    float alpha;
    float inv_frame_dt;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint SURFACE = 1u;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint k = thread.x;
    if (k >= pc.count) {
        return;
    }
    uint at = (pc.first + k) * 3u;
    float4 a = pc.spawns[at];
    float4 b = pc.spawns[at + 1u];
    uint4 c = asuint(pc.spawns[at + 2u]);
    float3 x = a.xyz;
    float3 v = b.xyz;
    if (c.y == SURFACE) {
        uint4 tri = pc.triangles[c.z];
        float3 w = float3(1.0 - a.x - a.y, a.x, a.y);
        float3 from = w.x * pc.skin_from[tri.x].xyz + w.y * pc.skin_from[tri.y].xyz + w.z * pc.skin_from[tri.z].xyz;
        float3 to = w.x * pc.skin_to[tri.x].xyz + w.y * pc.skin_to[tri.y].xyz + w.z * pc.skin_to[tri.z].xyz;
        float3 p0 = lerp(pc.skin_from[tri.x].xyz, pc.skin_to[tri.x].xyz, pc.alpha);
        float3 p1 = lerp(pc.skin_from[tri.y].xyz, pc.skin_to[tri.y].xyz, pc.alpha);
        float3 p2 = lerp(pc.skin_from[tri.z].xyz, pc.skin_to[tri.z].xyz, pc.alpha);
        float3 across = cross(p1 - p0, p2 - p0);
        float area = length(across);
        float3 normal = area > 0.0 ? across / area : float3(0.0, 1.0, 0.0);
        x = lerp(from, to, pc.alpha) + normal * a.z;
        v = b.xyz + normal * b.w + (to - from) * pc.inv_frame_dt;
    }
    uint slot = pc.base + k;
    pc.positions[slot] = float4(x, 0.0);
    pc.velocities[slot] = float4(v, a.w > 0.0 ? a.w : FOREVER);
    pc.phases[slot] = (c.x & SUBSTANCE_MASK) | FRESH;
    pc.warm[slot] = float4(0.0);
    pc.bond_counts[slot] = 0u;
    pc.anchors[slot] = uint4(NONE);
}
