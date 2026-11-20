import rikka.common;

struct RikkaVertexMorph {
    uint target;
    float3 position;
    float3 normal;
    uint pad;
};

struct Push {
    RikkaFrameRef frame;
    uint64_t positions_in;
    uint64_t attributes_in;
    uint64_t skins_in;
    uint64_t positions_out;
    uint64_t attributes_out;
    uint64_t morph_starts;
    uint64_t morph_deltas;
    uint64_t morph_weights;
    uint vertex_count;
    uint morph_count;

    uint bone_offset;
    uint flags;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint GROUP_SIZE = 64u;
static const uint RIKKA_SKIN_HAS_SKELETON = 1u << 0;

[shader("compute")]
[numthreads(64, 1, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint vertex = global_id.y * WorkgroupCount().x * GROUP_SIZE + global_id.x;
    if (vertex >= push.vertex_count) {
        return;
    }

    float3 position = ((RikkaPositionRef)push.positions_in)[vertex];
    RikkaAttribute packed = ((RikkaAttributeRef)push.attributes_in)[vertex];
    float3 normal = rikka_decode_octahedral(packed.normal);
    float4 tangent = rikka_decode_tangent(packed.tangent);

    if (push.morph_count > 0u) {
        uint first = ((RikkaIndexListRef)push.morph_starts)[vertex];
        uint last = ((RikkaIndexListRef)push.morph_starts)[vertex + 1u];
        for (uint at = first; at < last; ++at) {
            RikkaVertexMorph morph = ((RikkaVertexMorph*)push.morph_deltas)[at];
            if (morph.target >= push.morph_count) {
                continue;
            }
            float weight = ((RikkaFloatRef)push.morph_weights)[morph.target];
            position += morph.position * weight;
            normal += morph.normal * weight;
        }
    }

    if ((push.flags & RIKKA_SKIN_HAS_SKELETON) != 0u) {
        uint4 skin = ((RikkaSkinRef)push.skins_in)[vertex];

        uint joints_lo = skin.x;
        uint joints_hi = skin.y;
        uint weights_lo = skin.z;
        uint weights_hi = skin.w;
        uint indices[4] = {
            joints_lo & 0xffffu, joints_lo >> 16, joints_hi & 0xffffu, joints_hi >> 16 };
        float weights[4] = {
            float(weights_lo & 0xffffu) / 65535.0,
            float(weights_lo >> 16) / 65535.0,
            float(weights_hi & 0xffffu) / 65535.0,
            float(weights_hi >> 16) / 65535.0 };

        float4x4 blended = (float4x4)0.0;
        [unroll]
        for (int i = 0; i < 4; ++i) {
            if (weights[i] > 0.0) {
                blended += push.frame.bones[push.bone_offset + indices[i]] * weights[i];
            }
        }
        position = mul(blended, float4(position, 1.0)).xyz;
        float3x3 rotation = (float3x3)blended;
        normal = mul(rotation, normal);
        tangent = float4(mul(rotation, tangent.xyz), tangent.w);
    }

    ((RikkaPositionRef)push.positions_out)[vertex] = position;

    RikkaAttribute out_packed;
    out_packed.normal = rikka_encode_octahedral(normalize(normal));
    out_packed.tangent = rikka_encode_tangent(float4(normalize(tangent.xyz), tangent.w));
    out_packed.uv = packed.uv;
    out_packed.color = packed.color;
    ((RikkaAttributeRef)push.attributes_out)[vertex] = out_packed;
}
