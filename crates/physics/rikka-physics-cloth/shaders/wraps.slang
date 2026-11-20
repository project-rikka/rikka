import cloth.common;
import cloth.world;
import cloth.job;

static const float WRAP_REACH = 0.035;
struct Push {
    Jobs jobs;
    uint job;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    Job job = pc.jobs[pc.job];
    uint own = job.shapes_figure.z;
    uint t = thread.x;
    if (t >= job.range.w) {
        return;
    }
    uint4 corners = job.triangles[t];
    float3 a = job.epoch_start[corners.x].xyz;
    float3 b = job.epoch_start[corners.y].xyz;
    float3 c = job.epoch_start[corners.z].xyz;
    float3 n = face_normal(a, b, c);
    float keys[WRAP_CANDIDATES];
    uint ids[WRAP_CANDIDATES];
    [ForceUnroll]
    for (uint k = 0u; k < WRAP_CANDIDATES; ++k) {
        keys[k] = FOREVER;
        ids[k] = NONE;
    }
    float radius = min(min(job.traits[corners.x].b.w, job.traits[corners.y].b.w), job.traits[corners.z].b.w);
    if (any(n != float3(0.0)) && radius >= 0.0) {
        uint particles[3] = { corners.x, corners.y, corners.z };
        for (uint p = 0u; p < 3u; ++p) {
            for (uint k = 0u; k < BODY_CANDIDATES; ++k) {
                uint entry = job.contacts[particles[p] * CONTACT_WORDS + k];
                if (entry == NONE || obstacle_figure(entry) != own) {
                    continue;
                }
                uint4 body = job.body_triangles[obstacle_index(entry)];
                float3 ba = job.body_from[body.x].xyz;
                float3 bb = job.body_from[body.y].xyz;
                float3 bc = job.body_from[body.z].xyz;
                float3 body_normal = face_normal(ba, bb, bc);
                float facing = dot(n, body_normal);
                uint vertices[3] = { body.x, body.y, body.z };
                for (uint m = 0u; m < 3u; ++m) {
                    float3 v = job.body_from[vertices[m]].xyz;
                    float d = dot(v - a, n);
                    float3 projected = v - n * d;
                    Closest closest = closest_on_triangle(projected, a, b, c);
                    if (!closest.face || abs(d) > WRAP_REACH) {
                        continue;
                    }
                    bool above = facing < 0.0;
                    float crossed = above ? max(-d, 0.0) : max(d, 0.0);
                    if (abs(facing) < 0.5 || crossed > (above ? radius : 0.0)) {
                        continue;
                    }
                    float key = abs(d);
                    uint id = vertices[m] | (above ? ABOVE : 0u);
                    [ForceUnroll]
                    for (uint s = 0u; s < WRAP_CANDIDATES; ++s) {
                        bool repeated = (id & INDEX_MASK) == (ids[s] & INDEX_MASK) && ids[s] != NONE;
                        bool before = key < keys[s] || (key == keys[s] && id < ids[s]);
                        float kept_key = keys[s];
                        uint kept_id = ids[s];
                        keys[s] = before ? key : kept_key;
                        ids[s] = before ? id : kept_id;
                        key = repeated ? FOREVER : (before ? kept_key : key);
                        id = repeated ? NONE : (before ? kept_id : id);
                    }
                }
            }
        }
    }
    for (uint k = 0u; k < WRAP_CANDIDATES; ++k) {
        job.wraps[t * WRAP_CANDIDATES + k] = ids[k];
    }
    if (ids[0] == NONE) {
        uint4 dests = job.wrap_dests[t];
        job.slots[dests.x] = float4(0.0);
        job.slots[dests.y] = float4(0.0);
        job.slots[dests.z] = float4(0.0);
    }
}
