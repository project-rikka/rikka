import cloth.common;

struct Anchor : INonWritable {
    uint4 ids;
    float4 weights;
    float4 extra;

    static Anchor load_from<let ALIGN : int>(Anchor* address) {
        return {
            load_non_writable<16>(__getAddress(address->ids)),
            load_non_writable<16>(__getAddress(address->weights)),
            load_non_writable<16>(__getAddress(address->extra)),
        };
    }
};

typealias Anchors = NonWritableBuffer<Anchor, 16>;

struct Push {
    ReadVec4s rest;
    Anchors anchors;
    Mat4s palettes;
    ReadVec4s body;
    ReadUvec4s body_triangles;
    Uvec4s targets;
    uint count;
    uint boundaries;
    uint slots;
    uint body_vertices;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint at = thread.x;
    if (at >= pc.count * pc.boundaries) {
        return;
    }
    uint boundary = at / pc.count;
    uint particle = at - boundary * pc.count;
    Anchor anchor = pc.anchors[particle];
    float3 position;
    float3 normal;
    if (uint(anchor.extra.w) == SURFACE) {
        uint4 corners = pc.body_triangles[anchor.ids.x];
        uint base = boundary * pc.body_vertices;
        float3 a = pc.body[base + corners.x].xyz;
        float3 b = pc.body[base + corners.y].xyz;
        float3 c = pc.body[base + corners.z].xyz;
        float3 face = cross(b - a, c - a);
        float area = dot(face, face);
        normal = area > 1.0e-30 ? face * rsqrt(area) : float3(0.0, 1.0, 0.0);
        float u = anchor.weights.x;
        float v = anchor.weights.y;
        float3 point = a * (1.0 - u - v) + b * u + c * v;
        position = point + mul(anchor.extra.xyz, triangle_frame(a, b, c, normal));
    } else {
        uint base = boundary * pc.slots;
        float4x4 blended = scaled(pc.palettes[base + anchor.ids.x].columns, anchor.weights.x);
        blended += scaled(pc.palettes[base + anchor.ids.y].columns, anchor.weights.y);
        blended += scaled(pc.palettes[base + anchor.ids.z].columns, anchor.weights.z);
        blended += scaled(pc.palettes[base + anchor.ids.w].columns, anchor.weights.w);
        position = mul(float4(pc.rest[particle].xyz, 1.0), blended).xyz;
        float3 turned = mul(anchor.extra.xyz, float3x3(blended));
        float length_squared = dot(turned, turned);
        normal = length_squared > 1.0e-30 ? turned * rsqrt(length_squared) : float3(0.0);
    }
    pc.targets[at] = uint4(asuint(position), pack_direction(normal));
}
