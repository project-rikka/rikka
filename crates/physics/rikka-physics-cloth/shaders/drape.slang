import cloth.common;
import cloth.world;
import cloth.bind;
import cloth.draping;

static const uint MAX_STRAND_NODES = 16u;
static const uint PUSH_PLANES = 16u;
static const uint MAX_DRAPE_COLLIDERS = 12u;
static const float PUSH_FACING = 0.1;
static const float MIN_DRAPE_GAP = -0.006;
static const float EDGE_NEAR_MARGIN = 0.007;
struct StrandNode : INonWritable {
    uint4 ids;
    float4 tangent;
    float4 bitangent;

    static StrandNode load_from<let ALIGN : int>(StrandNode* address) {
        return {
            load_non_writable<16>(__getAddress(address->ids)),
            load_non_writable<16>(__getAddress(address->tangent)),
            load_non_writable<16>(__getAddress(address->bitangent)),
        };
    }
};

typealias StrandNodes = NonWritableBuffer<StrandNode, 16>;

struct DrapeVertex {
    float3 position;
    float blend;
    float3 normal;
    float pad;
    float4 tangent;
};

typealias DrapeVertices = Buffer<DrapeVertex, 16>;
typealias Outputs = NonWritableBuffer<DrapeVertices, 8>;

struct Push {
    ReadVec4s positions;
    ReadVec4s normals;
    ReadUvec4s triangles;
    Binds binds;
    StrandNodes strand_nodes;
    Outputs outputs;
    ReadFloats hands;
    ReadUints nearest;
    ReadVec4s body;
    ReadUvec4s body_triangles;
    Colliders colliders;
    ReadVec4s planes;
    uint count;
    uint collider_count;
    float up_x;
    float up_y;
    float up_z;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

void body_planes_for(inout Planes<PUSH_PLANES> planes, inout uint seen[PUSH_PLANES], float3 p, float3 base, uint3 home, float required, bool faced) {
    uint particles[3] = { home.x, home.y, home.z };
    float3 facing = faced ? pc.normals[home.x].xyz + pc.normals[home.y].xyz + pc.normals[home.z].xyz : float3(0.0);
    float bar = PUSH_FACING * length(facing);
    for (uint k = 0u; k < 3u * BODY_NEAREST && planes.count < PUSH_PLANES; ++k) {
        uint entry = pc.nearest[NEAREST_WORDS * particles[k / BODY_NEAREST] + k % BODY_NEAREST];
        if (entry == NONE || (entry & (REMEMBERED | ABOVE)) == REMEMBERED) {
            continue;
        }
        uint tri_id = obstacle_index(entry);
        bool dup = false;
        for (uint s = 0u; s < planes.count; ++s) {
            if (seen[s] == tri_id) {
                dup = true;
                break;
            }
        }
        if (dup) {
            continue;
        }
        uint4 body = pc.body_triangles[tri_id];
        float3 a = pc.body[body.x].xyz;
        float3 b = pc.body[body.y].xyz;
        float3 c = pc.body[body.z].xyz;
        float3 n = face_normal(a, b, c);
        float toward = dot(n, facing);
        bool opposed = faced && toward <= -bar;
        if (all(n == float3(0.0)) || (faced && !opposed && toward < bar)) {
            continue;
        }
        float d = dot(p - a, n);
        bool in_front = !opposed || dot(base - a, n) > 0.0;
        Closest closest = closest_on_triangle(p, a, b, c);
        bool edge_near = !closest.face && !opposed && toward >= bar
            && d < required
            && distance(p, closest.point) < max(required, 0.0) + EDGE_NEAR_MARGIN;
        if ((closest.face || edge_near) && in_front && d > -PUSH_REACH) {
            seen[planes.count] = tri_id;
            planes.add(float4(n, dot(n, a) + required));
        }
    }
}

void collect_colliders(inout uint checked[MAX_DRAPE_COLLIDERS], inout uint checked_count, uint3 home) {
    uint particles[3] = { home.x, home.y, home.z };
    for (uint s = 0u; s < 3u && checked_count < MAX_DRAPE_COLLIDERS; ++s) {
        uint packed = pc.nearest[NEAREST_WORDS * particles[s] + COLLIDER_NEAREST];
        if (packed == NONE) {
            continue;
        }
        for (uint k = 0u; k < 2u && checked_count < MAX_DRAPE_COLLIDERS; ++k) {
            uint index = (packed >> (16u * k)) & 0xffffu;
            if (index == NO_COLLIDER || index >= pc.collider_count) {
                continue;
            }
            bool dup = false;
            for (uint c = 0u; c < checked_count; ++c) {
                if (checked[c] == index) {
                    dup = true;
                    break;
                }
            }
            if (!dup) {
                checked[checked_count] = index;
                checked_count += 1u;
            }
        }
    }
}

bool apply_collider_push(inout float3 p, uint checked[MAX_DRAPE_COLLIDERS], uint checked_count, float floor_gap) {
    bool pushed = false;
    for (uint c = 0u; c < checked_count; ++c) {
        float3 outward;
        float2 found = collider_distance(pc.colliders[checked[c]], pc.planes, p, outward);
        if (found.x < floor_gap) {
            p += outward * (floor_gap - found.x);
            pushed = true;
        }
    }
    return pushed;
}

float3x3 spanned(float3 along, float3 side) {
    float3 tangent = unit_or(along, float3(0.0, -1.0, 0.0));
    float3 bitangent = unit_or(side - tangent * dot(tangent, side), any_orthonormal(tangent));
    return float3x3(tangent, bitangent, cross(tangent, bitangent));
}

Placed on_strand(Draping draping, Bind bind) {
    float3 axis = float3(0.0);
    float3x3 turn = float3x3(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (uint k = 0u; k < MAX_STRAND_NODES; ++k) {
        float weight = asfloat(bind.target[MAX_STRAND_NODES / 4u + k / 4u][k % 4u]);
        if (weight == 0.0) {
            break;
        }
        StrandNode strand_node = pc.strand_nodes[bind.target[k / 4u][k % 4u]];
        float3 origin = draping.particle(strand_node.ids.x);
        float3x3 now = spanned(draping.particle(strand_node.ids.w) - draping.particle(strand_node.ids.z), draping.particle(strand_node.ids.y) - origin);
        float3 tangent = strand_node.tangent.xyz;
        float3 bitangent = strand_node.bitangent.xyz;
        float3x3 rest = float3x3(tangent, bitangent, cross(tangent, bitangent));
        axis += weight * origin;
        turn += scaled(mul(transpose(rest), now), weight);
    }
    float3x3 carried = spanned(turn[0], turn[1]);
    float4 directions = asfloat(bind.target[MAX_STRAND_NODES / 2u + 1u]);
    StrandNode leader = pc.strand_nodes[bind.target[0].x];
    return Placed(
        axis + mul(asfloat(bind.target[MAX_STRAND_NODES / 2u].xyz), carried),
        axis,
        mul(unpack_octahedral(directions.xy), carried),
        mul(unpack_octahedral(directions.zw), carried),
        leader.ids.xyw);
}

float3 raised(Bind bind) {
    float3 lift = float3(0.0);
    for (uint k = 0u; k < 3u; ++k) {
        uint4 place = bind.target[3u * k];
        float weight = asfloat(place.w);
        if (weight == 0.0) {
            continue;
        }
        uint4 corners = pc.triangles[place.x];
        float u = asfloat(place.y);
        float v = asfloat(place.z);
        float w = 1.0 - u - v;
        float4 na = pc.normals[corners.x];
        float4 nb = pc.normals[corners.y];
        float4 nc = pc.normals[corners.z];
        float height = na.w * w + nb.w * u + nc.w * v;
        lift += weight * height * unit_or(na.xyz * w + nb.xyz * u + nc.xyz * v, float3(0.0));
    }
    return lift;
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint at = thread.x;
    if (at >= pc.count) {
        return;
    }
    Draping draping = { pc.positions, pc.normals, pc.triangles, float3(pc.up_x, pc.up_y, pc.up_z) };
    Bind bind = pc.binds[at];
    bool strand = (bind.header.x & STRAND) != 0u;
    Placed placed = strand ? on_strand(draping, bind) : draping.on_triangles(bind);
    Planes<PUSH_PLANES> planes = Planes<PUSH_PLANES>();
    if (!strand) {
        placed.position += raised(bind);
    }
    float required = asfloat(bind.header.w);
    if (required >= MIN_DRAPE_GAP) {
        uint seen[PUSH_PLANES] = {};
        body_planes_for(planes, seen, placed.position, placed.base, placed.home, required, !strand);
        if (!strand) {
            for (uint k = 1u; k < 3u && planes.count < PUSH_PLANES; ++k) {
                uint4 place = bind.target[3u * k];
                if (asfloat(place.w) > 0.0) {
                    body_planes_for(planes, seen, placed.position, placed.base, pc.triangles[place.x].xyz, required, true);
                }
            }
        }
    }
    float3 position = pushed_out(placed.position, planes);
    if (required >= MIN_DRAPE_GAP && pc.collider_count > 0u) {
        float floor_gap = min(required, 0.0);
        uint checked[MAX_DRAPE_COLLIDERS] = {};
        uint checked_count = 0u;
        collect_colliders(checked, checked_count, placed.home);
        if (!strand) {
            for (uint k = 1u; k < 3u && checked_count < MAX_DRAPE_COLLIDERS; ++k) {
                uint4 place = bind.target[3u * k];
                if (asfloat(place.w) > 0.0) {
                    collect_colliders(checked, checked_count, pc.triangles[place.x].xyz);
                }
            }
        }
        if (apply_collider_push(position, checked, checked_count, floor_gap) && checked_count > 1u) {
            apply_collider_push(position, checked, checked_count, floor_gap);
        }
    }
    float3 n = unit_or(placed.normal, float3(0.0, 1.0, 0.0));
    float3 t = unit_or(placed.tangent - n * dot(placed.tangent, n), float3(0.0));
    DrapeVertices output = pc.outputs[bind.header.x & ~STRAND];
    output[bind.header.y] = DrapeVertex(
        position,
        asfloat(bind.header.z),
        n,
        0.0,
        float4(t, pc.hands[at]));
}
