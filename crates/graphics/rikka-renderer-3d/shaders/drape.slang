import rikka.common;
import rikka.drape;

struct Push {
    uint64_t positions;
    uint64_t attributes;
    uint64_t drape;
    float3x4 to_local;
    float3x3 linear;
    uint vertex_count;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint GROUP_SIZE = 64u;

float3 rikka_drape_direction(float3 skinned, float3 draped, float blend) {
    if (!(dot(draped, draped) > 1e-12)) {
        return skinned;
    }
    float3 toward = normalize(draped);
    float3 mixed = lerp(skinned, toward, blend);
    return dot(mixed, mixed) > 1e-12 ? normalize(mixed) : toward;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint vertex = global_id.y * WorkgroupCount().x * GROUP_SIZE + global_id.x;
    if (vertex >= push.vertex_count) {
        return;
    }
    RikkaDrapeVertex draped = ((RikkaDrapeVertex*)push.drape)[vertex];
    float blend = clamp(draped.blend, 0.0, 1.0);
    if (!(blend > 0.0)) {
        return;
    }

    float3* positions = (float3*)push.positions;
    float3 local = mul(push.to_local, float4(draped.position, 1.0));
    positions[vertex] = lerp(positions[vertex], local, blend);

    RikkaAttribute* attributes = (RikkaAttribute*)push.attributes;
    RikkaAttribute packed = attributes[vertex];
    float3 normal = rikka_decode_octahedral(packed.normal);
    float4 tangent = rikka_decode_tangent(packed.tangent);
    float3 draped_normal = mul(draped.normal, push.linear);
    float3 draped_tangent = mul((float3x3)push.to_local, draped.tangent.xyz);
    normal = rikka_drape_direction(normal, draped_normal, blend);
    tangent = float4(rikka_drape_direction(tangent.xyz, draped_tangent, blend), draped.tangent.w);
    packed.normal = rikka_encode_octahedral(normal);
    packed.tangent = rikka_encode_tangent(tangent);
    attributes[vertex] = packed;
}
