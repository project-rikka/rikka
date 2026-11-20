import rikka.common;
import rikka.editor;

struct Push {
    RikkaFrameRef frame;
    uint64_t edges;
    uint instance;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

struct EdgeVaryings {
    [[vk::location(0)]] float3 world;
};

static const float4 CULLED = float4(0.0, 0.0, -2.0, 1.0);
static const float FACING_EPSILON = 1e-5;
static const float NEARER = 1e-3;
static const uint CORNER_END[6] = { 0u, 0u, 1u, 1u, 0u, 1u };
static const float CORNER_SIDE[6] = { -1.0, 1.0, -1.0, -1.0, 1.0, 1.0 };

float3 incident(RikkaViewRef view, float3 view_position) {
    return view.projection[3][3] < 0.5 ? normalize(-view_position) : float3(0.0, 0.0, 1.0);
}

float3 unit(float3 v) {
    float length_squared = dot(v, v);
    return length_squared > 0.0 ? v * rsqrt(length_squared) : float3(0.0);
}

bool silhouette(RikkaViewRef view, float3 p0, float3 p1, float3 p2, float3 p3) {
    float3 toward = -incident(view, p1);
    float3 n0 = unit(cross(p2 - p1, p0 - p1));
    float3 n3 = unit(cross(p3 - p1, p2 - p1));
    float facing0 = dot(toward, n0);
    float facing3 = dot(toward, n3);
    if (abs(facing0) > FACING_EPSILON && abs(facing3) > FACING_EPSILON) {
        return sign(facing0) != sign(facing3);
    }
    return true;
}

[shader("vertex")]
EdgeVaryings vertex_main(uint vertex_index: SV_VulkanVertexID, out float4 position: SV_Position) {
    RikkaFrameRef frame = push.frame;
    RikkaViewRef view = frame.view;
    RikkaInstance inst = frame.instances[push.instance];
    uint edge = vertex_index / 6u;
    uint corner = vertex_index % 6u;
    uint4 quad = ((uint4*)push.edges)[edge] + uint4(inst.vertex_offset);
    RikkaPositionRef positions = (RikkaPositionRef)inst.positions;
    float3 world[4];
    float3 seen[4];
    for (uint at = 0u; at < 4u; at++) {
        float3 local = positions[quad[at]];
        world[at] = mul(inst.transform, float4(local, 1.0)).xyz;
        seen[at] = mul(view.view, float4(world[at], 1.0)).xyz;
    }

    EdgeVaryings out;
    out.world = world[1];
    if (!silhouette(view, seen[0], seen[1], seen[2], seen[3])) {
        position = CULLED;
        return out;
    }
    float4 start = mul(view.view_projection, float4(world[1], 1.0));
    float4 end = mul(view.view_projection, float4(world[2], 1.0));
    float3 start_world = world[1];
    float3 end_world = world[2];
    float start_ahead = start.w - start.z;
    float end_ahead = end.w - end.z;
    if (start_ahead < 0.0 && end_ahead < 0.0) {
        position = CULLED;
        return out;
    }
    if (start_ahead < 0.0) {
        float t = start_ahead / (start_ahead - end_ahead);
        start = lerp(start, end, t);
        start_world = lerp(start_world, end_world, t);
    } else if (end_ahead < 0.0) {
        float t = end_ahead / (end_ahead - start_ahead);
        end = lerp(end, start, t);
        end_world = lerp(end_world, start_world, t);
    }
    if (!(start.w > 0.0) || !(end.w > 0.0)) {
        position = CULLED;
        return out;
    }
    float2 from = (start.xy / start.w * 0.5 + 0.5) * view.viewport;
    float2 to = (end.xy / end.w * 0.5 + 0.5) * view.viewport;
    float2 delta = to - from;
    float span = length(delta);
    float2 direction = span > 1e-4 ? delta / span : float2(1.0, 0.0);
    float2 normal = float2(-direction.y, direction.x);
    bool at_end = CORNER_END[corner] == 1u;
    float4 clip = at_end ? end : start;
    float2 offset = normal * (CORNER_SIDE[corner] * 0.5);
    float nearer = min(clip.z + NEARER * clip.w, clip.w);
    position = float4(clip.xy + offset * 2.0 * view.inverse_viewport * clip.w, nearer, clip.w);
    out.world = at_end ? end_world : start_world;
    return out;
}

[shader("fragment")]
uint fragment_main(EdgeVaryings varyings) : SV_Target {
    if (rikka_clipped(push.frame, varyings.world)) {
        discard;
    }
    RikkaInstance inst = push.frame.instances[push.instance];
    return (inst.outline << RIKKA_OUTLINE_COLOR_SHIFT) | ((inst.object_id + 1u) & RIKKA_OUTLINE_OBJECT_MASK);
}
