import fluid;
import fluid.collision;
import physics.reaction;

struct Push {
    Settings settings;
    Vec4s positions;
    Vec4s predicted;
    Uints phases;
    Vec4s velocities;
    Vec4s motion;
    BodyTableRef bodies;
    Uints cache;
    Vec4s contacts;
    Uvec2s walls;
    Floats clearances;
    Uvec4s anchors;
    uint count;
    float dt;
    uint mode;
    uint stride;
    float max_speed;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint LAST = 1u;
static const uint PROBE = 2u;
static const uint SHOWN = 4u;
static const uint REACT = 8u;
static const float ANCHOR_DEPTH = 0.3;

uint remember(BodyTable table, uint t, float3 x0, float reach, float margin, bool fresh) {
    if (fresh) {
        return t;
    }
    uint at = t * FACE_VECS;
    float4 start = table.faces[at + 6u];
    if (all(start.xyz == float3(0.0))) {
        return t;
    }
    float d0 = dot(x0, start.xyz) - start.w;
    if (abs(d0) >= reach) {
        return t;
    }
    for (uint k = 0u; k < 3u; ++k) {
        float4 edge = table.faces[at + 7u + k];
        if (dot(x0, edge.xyz) - edge.w < -margin) {
            return t;
        }
    }
    return t | REMEMBERED | (d0 >= 0.0 ? ABOVE : 0u);
}

float probe(BodyTable table, Settings s, uint i, float3 x0, float3 x, bool fresh) {
    float thickness = s.body.x;
    float near_reach = thickness + s.body.y + s.tri_cell.z + 0.5 * kernel_h(s);
    float reach = near_reach + length(x - x0);
    float keys[MAX_CANDIDATES];
    uint entries[MAX_CANDIDATES];
    [ForceUnroll]
    for (uint k = 0u; k < MAX_CANDIDATES; ++k) {
        keys[k] = FOREVER;
        entries[k] = NONE;
    }
    uint bin = bin_of(cell_of(x, s.tri_cell.y), s.tri_grid);
    uint first = table.tri_starts[bin];
    uint end = table.tri_starts[bin + 1u];
    for (uint e = first; e < end; ++e) {
        uint t = table.tri_items[e];
        float4 sphere = table.spheres[2u * t];
        float4 plane = table.spheres[2u * t + 1u];
        float near = length(x - sphere.xyz) - sphere.w;
        if (max(near, abs(dot(x, plane.xyz) - plane.w)) > near_reach || near > keys[MAX_CANDIDATES - 1u]) {
            continue;
        }
        float key = near;
        uint id = t;
        [ForceUnroll]
        for (uint k = 0u; k < MAX_CANDIDATES; ++k) {
            bool repeated = id == entries[k];
            bool before = key < keys[k] || (key == keys[k] && id < entries[k]);
            float kept_key = keys[k];
            uint kept_id = entries[k];
            keys[k] = before ? key : kept_key;
            entries[k] = before ? id : kept_id;
            key = repeated ? FOREVER : (before ? kept_key : key);
            id = repeated ? NONE : (before ? kept_id : id);
        }
    }
    float far = entries[MAX_CANDIDATES - 1u] == NONE ? near_reach : keys[MAX_CANDIDATES - 1u];
    [ForceUnroll]
    for (uint k = 0u; k < MAX_CANDIDATES; ++k) {
        uint entry = entries[k] == NONE ? NONE : remember(table, entries[k], x0, reach, thickness, fresh);
        pc.cache[k * pc.stride + i] = entry;
        if (entry == NONE) {
            break;
        }
    }
    return far;
}

void deposit(BodyTable table, Substance kind, Touch support) {
    uint4 tri = table.triangles[support.triangle];
    float amount = kind.tint.w * pc.dt;
    uint corners[3] = { tri.x, tri.y, tri.z };
    for (uint k = 0u; k < 3u; ++k) {
        float share = amount * support.weights[k];
        if (share <= 0.0) {
            continue;
        }
        uint at = corners[k] * 4u;
        InterlockedAdd(*table.wet.at(at), uint(share * WET_UNIT + 0.5));
        InterlockedAdd(*table.wet.at(at + 1u), uint(share * kind.tint.x * WET_UNIT + 0.5));
        InterlockedAdd(*table.wet.at(at + 2u), uint(share * kind.tint.y * WET_UNIT + 0.5));
        InterlockedAdd(*table.wet.at(at + 3u), uint(share * kind.tint.z * WET_UNIT + 0.5));
    }
}

void react(BodyTable table, Settings s, Touch support, uint i, float3 x0, float3 x) {
    float3 expected = pc.velocities[i].xyz + s.reaction.xyz * pc.dt;
    float pushed = dot((x - x0) / pc.dt - expected, support.normal);
    if (pushed <= 0.0) {
        return;
    }
    float3 impulse = support.normal * (-pushed * s.reaction.w);
    uint4 tri = table.triangles[support.triangle];
    uint corners[3] = { tri.x, tri.y, tri.z };
    for (uint k = 0u; k < 3u; ++k) {
        int3 share = int3(round(impulse * support.weights[k]));
        if (all(share == int3(0))) {
            continue;
        }
        uint at = corners[k] * REACTION_WORDS;
        InterlockedAdd(*table.reactions.at(at), share.x);
        InterlockedAdd(*table.reactions.at(at + 1u), share.y);
        InterlockedAdd(*table.reactions.at(at + 2u), share.z);
    }
}

void finish(uint i, uint phase, float3 x0, float swirl, float3 x) {
    Settings s = pc.settings;
    float3 v = (x - x0) / pc.dt;
    float speed = length(v);
    if (speed > pc.max_speed) {
        v *= pc.max_speed / speed;
        x = x0 + v * pc.dt;
    }
    float life = pc.velocities[i].w - pc.dt;
    bool outside = outside_bounds(s, x);
    pc.phases[i] = (outside || life <= 0.0) ? (phase | DEAD) : (phase & ~FRESH);
    pc.positions[i] = float4(x, swirl);
    pc.velocities[i] = float4(v, life);
    if ((pc.mode & SHOWN) != 0u) {
        pc.motion[i] = float4(v, 0.0);
    }
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint phase = pc.phases[i];
    Settings s = pc.settings;
    bool last = (pc.mode & LAST) != 0u;
    bool probing = (pc.mode & PROBE) != 0u;
    bool bodies = s.tri_grid.w > 0u;
    if (is_dead(phase)) {
        if (last) {
            pc.contacts[i] = float4(0.0);
        }
        if (probing && bodies) {
            pc.cache[i] = NONE;
        }
        if (!last) {
            pc.walls[i] = pack_plane(float4(0.0, 1.0, 0.0, -FOREVER), float3(0.0));
            pc.clearances[i] = FOREVER;
        }
        return;
    }
    float thickness = s.body.x;
    float band = s.body.y;
    float4 start = pc.positions[i];
    float3 x0 = start.xyz;
    float3 x = pc.predicted[i].xyz;
    bool walled = s.body.w > 0.5 || bodies;
    bool touches = walled && (probing || pc.clearances[i] < thickness + band);
    float4 contact = float4(0.0);
    if (touches) {
        BodyTable table = pc.bodies.value;
        Substance kind = s.substances[substance_of(phase)];
        float far = FOREVER;
        if (probing && bodies) {
            far = probe(table, s, i, x0, x, (phase & FRESH) != 0u);
        }
        Pushed pushed = push_out(s, table.faces, table.spheres, pc.cache, pc.stride, i, x, x0, kind.contact.x);
        x = pushed.x;
        Touch support = pushed.support;
        float distance = max(support.gap - thickness, 0.0);
        if (support.triangle != NONE) {
            x -= support.normal * min(distance, kind.stick.z * pc.dt * pc.dt);
        }
        uint4 anchor = bodies ? pc.anchors[i] : uint4(NONE);
        uint4 was = anchor;
        x = pull_to_anchor(table, anchor, x, kind);
        Touch nearest = pushed.nearest;
        if (anchor.x == NONE && kind.stick.y > 0.0 && nearest.triangle != NONE && nearest.gap < thickness + ANCHOR_DEPTH * kernel_h(s)) {
            anchor = anchored_at(nearest, table.spheres[2u * nearest.triangle + 1u].xyz, max(nearest.gap, thickness));
        }
        if (any(anchor != was)) {
            pc.anchors[i] = anchor;
        }
        uint pinned = anchor.x != NONE ? (phase | PINNED) : (phase & ~PINNED);
        if (pinned != phase) {
            phase = pinned;
            if (!last) {
                pc.phases[i] = phase;
            }
        }
        if (last) {
            contact.w = pushed.grounded;
            if (pushed.grounded > 0.0) {
                x = dragged(x0, x, float3(0.0, 1.0, 0.0), float3(0.0), kind.contact.y * pushed.grounded * pc.dt);
            }
            if (support.triangle != NONE) {
                float falloff = 1.0 - clamp(distance / band, 0.0, 1.0);
                float3 travel = travel_of(table.faces, support);
                x = dragged(x0, x, support.normal, travel, (anchor.x == NONE ? kind.stick.w + kind.contact.y : kind.contact.y) * falloff * pc.dt);
                if (falloff > contact.w) {
                    contact = float4(travel / pc.dt, falloff);
                }
                if (support.gap < thickness + 0.5 * band && kind.tint.w > 0.0) {
                    deposit(table, kind, support);
                }
                if ((pc.mode & REACT) != 0u) {
                    react(table, s, support, i, x0, x);
                }
            }
        } else {
            pc.walls[i] = pack_plane(pushed.plane, x);
            pc.clearances[i] = probing && anchor.x == NONE ? min(pushed.clearance, far) : 0.0;
            pc.predicted[i] = float4(x, 0.0);
        }
    }
    if (last) {
        pc.contacts[i] = contact;
        finish(i, phase, x0, start.w, x);
    }
}
