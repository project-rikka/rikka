import cloth.common;

struct Push {
    ReadVec4s positions;
    ReadUvec4s triangles;
    ReadUints incident_starts;
    ReadUints incident;
    Vec4s normals;
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
    float3 sum = float3(0.0);
    uint to = pc.incident_starts[i + 1u];
    for (uint at = pc.incident_starts[i]; at < to; ++at) {
        uint4 corners = pc.triangles[pc.incident[at]];
        float3 a = pc.positions[corners.x].xyz;
        float3 b = pc.positions[corners.y].xyz;
        float3 c = pc.positions[corners.z].xyz;
        sum += cross(b - a, c - a);
    }
    float length_squared = dot(sum, sum);
    float3 normal = length_squared > 1.0e-30 ? sum * rsqrt(length_squared) : float3(0.0);
    pc.normals[i] = float4(normal, 0.0);
}
