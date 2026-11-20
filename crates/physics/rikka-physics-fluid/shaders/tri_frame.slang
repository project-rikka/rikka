import fluid;

struct Push {
    ReadVec4s skin_from;
    ReadVec4s skin_to;
    ReadUvec4s triangles;
    Vec4s spheres;
    Vec4s faces;
    uint count;
    float alpha0;
    float alpha1;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

float3 unit_normal(float3 a, float3 b, float3 c) {
    float3 across = cross(b - a, c - a);
    float area = length(across);
    return area > 0.0 ? across / area : float3(0.0);
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint t = thread.x;
    if (t >= pc.count) {
        return;
    }
    uint4 tri = pc.triangles[t];
    uint corners[3] = { tri.x, tri.y, tri.z };
    float3 starts[3];
    float3 ends[3];
    for (int k = 0; k < 3; ++k) {
        float3 from = pc.skin_from[corners[k]].xyz;
        float3 to = pc.skin_to[corners[k]].xyz;
        starts[k] = lerp(from, to, pc.alpha0);
        ends[k] = lerp(from, to, pc.alpha1);
    }
    float3 centre = float3(0.0);
    for (int k = 0; k < 3; ++k) {
        centre += starts[k] + ends[k];
    }
    centre *= 1.0 / 6.0;
    float radius = 0.0;
    for (int k = 0; k < 3; ++k) {
        radius = max(radius, max(length(starts[k] - centre), length(ends[k] - centre)));
    }
    float3 n1 = unit_normal(ends[0], ends[1], ends[2]);
    pc.spheres[2u * t] = float4(centre, radius);
    pc.spheres[2u * t + 1u] = float4(n1, dot(n1, ends[0]));
    float3 n0 = unit_normal(starts[0], starts[1], starts[2]);
    uint at = t * FACE_VECS;
    pc.faces[at + 6u] = float4(n0, dot(n0, starts[0]));
    for (int k = 0; k < 3; ++k) {
        pc.faces[at + uint(k)] = float4(ends[k], 0.0);
        pc.faces[at + 3u + uint(k)] = float4(ends[k] - starts[k], 0.0);
        float3 from = starts[k];
        float3 to = starts[(k + 1) % 3];
        float3 opposite = starts[(k + 2) % 3];
        float3 inward = cross(n0, to - from);
        float length_inward = length(inward);
        inward = length_inward > 0.0 ? inward / length_inward : float3(0.0);
        if (dot(opposite - from, inward) < 0.0) {
            inward = -inward;
        }
        pc.faces[at + 7u + uint(k)] = float4(inward, dot(inward, from));
    }
}
