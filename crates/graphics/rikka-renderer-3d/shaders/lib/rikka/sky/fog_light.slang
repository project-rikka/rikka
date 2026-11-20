__exported import rikka.sky.fog;
import rikka.light;

uint rikka_fog_shadow_slot(RikkaFrameRef frame, RikkaLight light, float3 position,
                           float view_depth) {
    if (light.kind == RIKKA_LIGHT_DIRECTIONAL) {
        for (uint i = 0u; i + 1u < light.shadow_count; ++i) {
            if (view_depth < frame.shadows[light.shadow + i].split_depth) {
                return light.shadow + i;
            }
        }
        return light.shadow + light.shadow_count - 1u;
    }
    if (light.kind != RIKKA_LIGHT_SPOT) {
        float3 d = position - light.position;
        float3 a = abs(d);
        if (a.x >= a.y && a.x >= a.z) {
            return light.shadow + (d.x >= 0.0 ? 0u : 1u);
        }
        if (a.y >= a.z) {
            return light.shadow + (d.y >= 0.0 ? 2u : 3u);
        }
        return light.shadow + (d.z >= 0.0 ? 4u : 5u);
    }
    return light.shadow;
}

float rikka_fog_visibility(RikkaFrameRef frame, RikkaLight light, float3 position,
                           float view_depth) {
    if (light.shadow == RIKKA_NONE || light.shadow_count == 0u) {
        return 1.0;
    }
    uint atlas = frame.view.shadow_atlas;
    if (atlas == RIKKA_NONE) {
        return 1.0;
    }
    RikkaShadowSlot slot = frame.shadows[rikka_fog_shadow_slot(frame, light, position,
                                                               view_depth)];
    if ((slot.flags & RIKKA_SHADOW_FIGURE) != 0u) {
        return 1.0;
    }
    float4 clip = mul(slot.view_projection, float4(position, 1.0));
    if (clip.w <= 0.0) {
        return 1.0;
    }
    float3 ndc = clip.xyz / clip.w;
    if (any(abs(ndc.xy) > float2(1.0)) || ndc.z <= 0.0) {
        return 1.0;
    }
    float2 lowest = slot.uv_offset + 0.5 * slot.texel_size;
    float2 highest = slot.uv_offset + slot.uv_scale - 0.5 * slot.texel_size;
    float2 uv = clamp((ndc.xy * 0.5 + 0.5) * slot.uv_scale + slot.uv_offset, lowest, highest);
    return rikka_images[atlas].SampleCmpLevelZero(
        rikka_shadow_samplers[RIKKA_SAMPLER_SHADOW], uv, ndc.z + slot.depth_bias);
}

float rikka_fog_attenuation(RikkaFrameRef frame, RikkaLight light, float3 position, out float3 to_light) {
    if (light.kind == RIKKA_LIGHT_DIRECTIONAL) {
        to_light = -light.direction;
        return 1.0;
    }
    float3 offset = light.position - position;
    float distance_squared = max(dot(offset, offset), 1e-8);
    float distance = sqrt(distance_squared);
    to_light = offset / distance;
    float ratio = distance / max(light.range, 1e-4);
    float window = clamp(1.0 - ratio * ratio * ratio * ratio, 0.0, 1.0);
    float attenuation = window * window / max(distance_squared, 1e-2) * rikka_light_profile(frame, light, -to_light);
    if (light.kind == RIKKA_LIGHT_SPOT) {
        float cone = clamp(dot(-to_light, light.direction) * light.cone_scale
                           + light.cone_offset, 0.0, 1.0);
        attenuation *= cone * cone;
    }
    return attenuation;
}

float3 rikka_fog_lights(RikkaFrameRef frame, RikkaSky sky, float3 position, float3 ray,
                        float view_depth) {
    RikkaViewRef view = frame.view;
    float3 light = float3(0.0);
    uint count = min(view.light_count, RIKKA_FOG_LIGHTS);
    for (uint i = 0u; i < count; ++i) {
        RikkaLight item = frame.lights[i];
        if (item.figure != 0u) {
            continue;
        }
        float3 to_light;
        float attenuation = rikka_fog_attenuation(frame, item, position, to_light);
        if (attenuation <= 0.0) {
            continue;
        }
        float weight = item.kind == RIKKA_LIGHT_DIRECTIONAL ? sky.fog_shafts : 1.0;
        float visible = rikka_fog_visibility(frame, item, position, view_depth);
        light += item.color * rikka_light_power(item) * attenuation * weight * visible
            * rikka_fog_phase(sky, dot(ray, to_light));
    }
    return light;
}
