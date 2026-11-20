import rikka.push;

static const uint GROUP_SIZE = 64u;
static const uint RIKKA_GROUP_LIMIT = 65535u;
static const uint RIKKA_LATE_DISPATCH = 1u;

bool occluded(RikkaViewRef view, float3 center, float radius) {
    if (view.hzb_image == RIKKA_NONE) {
        return false;
    }
    float3 to_center = center - view.camera_position;
    float depth = dot(to_center, view.camera_forward);
    if (depth - radius < view.near) {
        return false;
    }

    float2 min_uv = float2(1.0);
    float2 max_uv = float2(0.0);
    float nearest = 0.0;
    [unroll]
    for (int i = 0; i < 8; ++i) {
        float3 corner = center + radius * float3(
            (i & 1) != 0 ? 1.0 : -1.0,
            (i & 2) != 0 ? 1.0 : -1.0,
            (i & 4) != 0 ? 1.0 : -1.0);
        float4 clip = mul(view.view_projection, float4(corner, 1.0));
        if (clip.w <= 0.0) {
            return false;
        }
        float3 ndc = clip.xyz / clip.w;
        float2 uv = ndc.xy * 0.5 + 0.5;
        min_uv = min(min_uv, uv);
        max_uv = max(max_uv, uv);
        nearest = max(nearest, ndc.z);
    }
    min_uv = clamp(min_uv, float2(0.0), float2(1.0));
    max_uv = clamp(max_uv, float2(0.0), float2(1.0));

    float2 extent = (max_uv - min_uv) * view.hzb_size;
    float level = ceil(log2(max(max(extent.x, extent.y), 1.0)));
    level = clamp(level, 0.0, float(view.hzb_levels - 1u));

    Texture2D hzb = rikka_images[view.hzb_image];
    SamplerState reduce = rikka_samplers[RIKKA_SAMPLER_DEPTH_REDUCE];
    float furthest = 1.0;
    furthest = min(furthest, hzb.SampleLevel(reduce, float2(min_uv.x, min_uv.y), level).r);
    furthest = min(furthest, hzb.SampleLevel(reduce, float2(max_uv.x, min_uv.y), level).r);
    furthest = min(furthest, hzb.SampleLevel(reduce, float2(min_uv.x, max_uv.y), level).r);
    furthest = min(furthest, hzb.SampleLevel(reduce, float2(max_uv.x, max_uv.y), level).r);

    return nearest < furthest;
}

void planes_of(float4x4 matrix, out float4 planes[6]) {
    planes[0] = matrix[3] + matrix[0];
    planes[1] = matrix[3] - matrix[0];
    planes[2] = matrix[3] + matrix[1];
    planes[3] = matrix[3] - matrix[1];
    planes[4] = matrix[3] - matrix[2];
    planes[5] = matrix[2];
    [unroll]
    for (int i = 0; i < 6; ++i) {
        float length_of = length(planes[i].xyz);
        planes[i] = length_of > 1e-6 ? planes[i] / length_of : float4(0.0, 0.0, 0.0, 1e30);
    }
}

bool inside(float4 planes[6], float3 center, float radius) {
    [unroll]
    for (int i = 0; i < 6; ++i) {
        if (dot(planes[i].xyz, center) + planes[i].w < -radius) {
            return false;
        }
    }
    return true;
}

uint append(RikkaCounterRef counters, uint key) {
    uint slot = 0u;
    InterlockedAdd(counters[key], 1u, slot);
    return slot;
}

void emit(RikkaFrameRef frame, RikkaInstance inst, uint instance_index, uint bucket) {
    RikkaBucket into = frame.buckets[bucket];
    uint slot = append(frame.draw_counts, bucket);
    if (slot >= into.capacity) {
        return;
    }
    uint at = into.first + slot;
    frame.draw_commands[at].index_count = inst.index_count;
    frame.draw_commands[at].instance_count = 1u;
    frame.draw_commands[at].first_index = inst.first_index;
    frame.draw_commands[at].vertex_offset = int(inst.vertex_offset);
    frame.draw_commands[at].first_instance = instance_index;
}

void append_late_instance(RikkaFrameRef frame, uint instance_index) {
    uint at = 0u;
    InterlockedAdd(frame.late_counter[0], 1u, at);
    if (at % GROUP_SIZE == 0u) {
        InterlockedMax(frame.late_counter[RIKKA_LATE_DISPATCH], min(at / GROUP_SIZE + 1u, RIKKA_GROUP_LIMIT));
    }
    frame.late_instances[at] = instance_index;
}

void cull_camera(RikkaFrameRef frame, RikkaViewRef view, uint index) {
    RikkaInstance inst = frame.instances[index];
    if (inst.index_count == 0u || (inst.flags & RIKKA_DIRECT) != 0u) {
        return;
    }
    if (!rikka_sphere_in_frustum(view, inst.center, inst.radius)) {
        return;
    }
    if ((inst.flags & RIKKA_NO_OCCLUSION) == 0u && occluded(view, inst.center, inst.radius)) {
        append_late_instance(frame, index);
        return;
    }
    emit(frame, inst, index, inst.bucket);
}

void cull_late(RikkaFrameRef frame, RikkaViewRef view, uint index) {
    RikkaInstance inst = frame.instances[index];
    if (!occluded(view, inst.center, inst.radius)) {
        emit(frame, inst, index, rikka_push.slot + inst.bucket);
    }
}

bool casts_into(RikkaFrameRef frame, RikkaShadowSlot slot, RikkaInstance inst) {
    if ((slot.flags & RIKKA_SHADOW_CACHED) != 0u || (inst.light_channels & slot.channels) == 0u) {
        return false;
    }
    if ((slot.flags & RIKKA_SHADOW_FIGURE) != 0u) {
        return inst.figure == slot.figure;
    }
    if ((slot.flags & RIKKA_SHADOW_WITHOUT_MAPPED_FIGURES) != 0u && inst.figure != 0u) {
        return (frame.figures[inst.figure].flags & RIKKA_FIGURE_MAPPED) == 0u;
    }
    return true;
}

void cull_shadow(RikkaFrameRef frame, float4 planes[6], RikkaShadowSlot slot, uint shadow, uint index) {
    RikkaInstance inst = frame.instances[index];
    if (inst.index_count == 0u || (inst.flags & RIKKA_CASTS_SHADOW) == 0u) {
        return;
    }
    if (!casts_into(frame, slot, inst) || !inside(planes, inst.center, inst.radius)) {
        return;
    }
    emit(frame, inst, index, rikka_push.slot + shadow * frame.view.shadow_classes + inst.shadow_class);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID, uint3 group_id: SV_GroupID) {
    RikkaFrameRef frame = rikka_push.frame;
    RikkaViewRef view = frame.view;
    uint first = global_id.x;
    uint stride = WorkgroupCount().x * GROUP_SIZE;

    if ((rikka_push.flags & RIKKA_PASS_LATE) != 0u) {
        uint count = frame.late_counter[0];
        for (uint i = first; i < count; i += stride) {
            cull_late(frame, view, frame.late_instances[i]);
        }
        return;
    }

    if (group_id.z == 0u) {
        for (uint i = first; i < view.instance_count; i += stride) {
            cull_camera(frame, view, i);
        }
        return;
    }

    uint shadow = group_id.z - 1u;
    RikkaShadowSlot slot = frame.shadows[shadow];
    float4 planes[6];
    planes_of(slot.view_projection, planes);
    for (uint i = first; i < view.instance_count; i += stride) {
        cull_shadow(frame, planes, slot, shadow, i);
    }
}
