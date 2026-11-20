__exported import rikka.common;
__exported import rikka.shading;

static const uint RIKKA_RAY_SHADOW = 1u;
static const uint RIKKA_RAY_SEEN = 2u;
static const float RIKKA_RAY_TEXTURE_LEVEL = 2.0;
static const float RIKKA_RAY_CUTOFF = 0.5;
static const uint RIKKA_ALBEDO_ALPHA = 1u;
static const float RIKKA_RAY_DIELECTRIC_F0 = 0.04;

RaytracingAccelerationStructure rikka_ray_scene(uint64_t address) {
    return spirv_asm {
        result:$$RaytracingAccelerationStructure = OpConvertUToAccelerationStructureKHR $address
    };
}

uint3 rikka_ray_triangle(RikkaTrace trace, RikkaInstance inst, uint primitive) {
    uint first = inst.first_index + primitive * 3u;
    if ((inst.flags & RIKKA_INDEX16) != 0u) {
        uint* words = (uint*)rikka_address(trace.indices16);
        uint3 at = uint3(first, first + 1u, first + 2u);
        uint3 index;
        [unroll]
        for (int i = 0; i < 3; ++i) {
            uint word = words[at[i] >> 1u];
            index[i] = (at[i] & 1u) != 0u ? word >> 16u : word & 0xffffu;
        }
        return index;
    }
    uint* indices = (uint*)rikka_address(trace.indices32);
    return uint3(indices[first], indices[first + 1u], indices[first + 2u]);
}

struct RikkaRayVertex {
    float3 position;
    float3 normal;
    float2 uv;
    float4 color;
};

RikkaRayVertex rikka_ray_vertex(RikkaInstance inst, uint3 triangle, float2 barycentrics) {
    float3 weights = float3(1.0 - barycentrics.x - barycentrics.y, barycentrics.x, barycentrics.y);
    RikkaPositionRef positions = (RikkaPositionRef)inst.positions;
    RikkaAttributeRef attributes = (RikkaAttributeRef)inst.attributes;
    RikkaRayVertex v;
    v.position = float3(0.0);
    v.normal = float3(0.0);
    v.uv = float2(0.0);
    v.color = float4(0.0);
    float3 corners[3];
    [unroll]
    for (int i = 0; i < 3; ++i) {
        uint index = inst.vertex_offset + triangle[i];
        RikkaAttribute a = attributes[index];
        corners[i] = positions[index];
        v.position += corners[i] * weights[i];
        v.normal += rikka_decode_octahedral(a.normal) * weights[i];
        v.uv += rikka_decode_uv(a.uv) * weights[i];
        v.color += rikka_decode_color(a.color) * weights[i];
    }
    v.position = mul(inst.transform, float4(v.position, 1.0)).xyz;
    float3 flat = cross(corners[1] - corners[0], corners[2] - corners[0]);
    float3 smooth = dot(v.normal, v.normal) > 1e-12 ? v.normal : flat;
    v.normal = normalize(mul(inst.normal_matrix, smooth));
    return v;
}

uint64_t rikka_ray_block(RikkaFrameRef frame, RikkaMaterial material) {
    return frame.blocks + uint64_t(material.block) * uint64_t(RIKKA_BLOCK_ALIGNMENT);
}

float4 rikka_ray_sample(uint64_t block, uint offset, float2 uv, float level) {
    if (offset == RIKKA_NONE) {
        return float4(1.0);
    }
    uint2 texture = *(uint2*)(block + uint64_t(offset));
    if (texture.x == RIKKA_NONE) {
        return float4(1.0);
    }
    return rikka_images[NonUniformResourceIndex(texture.x)].SampleLevel(
        rikka_samplers[NonUniformResourceIndex(texture.y)], uv, level);
}

float4 rikka_ray_albedo(RikkaFrameRef frame, RikkaMaterial material, RikkaRayVertex v, float level) {
    uint64_t block = rikka_ray_block(frame, material);
    float4 albedo = float4(0.5, 0.5, 0.5, 1.0);
    if (material.albedo != RIKKA_NONE) {
        albedo = (material.roles & RIKKA_ALBEDO_ALPHA) != 0u
            ? *(float4*)(block + uint64_t(material.albedo))
            : float4(*(float3*)(block + uint64_t(material.albedo)), 1.0);
    }
    return albedo * v.color * rikka_ray_sample(block, material.albedo_texture, v.uv, level);
}

float rikka_ray_scalar(uint64_t block, uint offset, float fallback) {
    return offset == RIKKA_NONE ? fallback : *(float*)(block + uint64_t(offset));
}

float3 rikka_ray_emission(RikkaFrameRef frame, RikkaMaterial material, RikkaRayVertex v) {
    if (material.emission == RIKKA_NONE) {
        return float3(0.0);
    }
    uint64_t block = rikka_ray_block(frame, material);
    float3 emission = *(float3*)(block + uint64_t(material.emission));
    float scale = material.emission_luminance == RIKKA_NONE
        ? 1.0
        : rikka_ray_scalar(block, material.emission_luminance, 0.0) / RIKKA_NITS_PER_UNIT;
    return emission * scale * rikka_ray_sample(block, material.emission_texture, v.uv, RIKKA_RAY_TEXTURE_LEVEL).rgb;
}

float4 rikka_ray_response(RikkaFrameRef frame, RikkaMaterial material, RikkaRayVertex v, float level) {
    float4 base = rikka_ray_albedo(frame, material, v, level);
    uint64_t block = rikka_ray_block(frame, material);
    float weight = rikka_ray_scalar(block, material.albedo_weight, 1.0);
    float metal = clamp(rikka_ray_scalar(block, material.metalness, 0.0)
        * rikka_ray_sample(block, material.metalness_texture, v.uv, level).b, 0.0, 1.0);
    float clear = clamp(rikka_ray_scalar(block, material.transmission, 0.0), 0.0, 1.0);
    float3 diffuse = base.rgb * weight * (1.0 - metal) * (1.0 - clear);
    float3 specular = material.metalness == RIKKA_NONE
        ? float3(0.0)
        : lerp(float3(RIKKA_RAY_DIELECTRIC_F0), base.rgb * weight, metal);
    return float4(diffuse + specular, base.a);
}

uint rikka_ray_part(RikkaTrace trace, uint custom, uint geometry) {
    uint* parts = (uint*)rikka_address(trace.parts);
    return parts[custom + geometry];
}

RikkaInstance rikka_ray_instance(RikkaFrameRef frame, uint index, uint hook) {
    RikkaInstance inst = frame.instances[index];
    if (hook == 0u) {
        return inst;
    }
    RikkaHookJob job = ((RikkaHookJob*)rikka_address(frame.look.trace.hooks))[hook - 1u];
    inst.positions = rikka_address(job.positions);
    inst.attributes = rikka_address(job.attributes);
    inst.vertex_offset = 0u;
    inst.transform = float4x4(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    inst.normal_matrix = float3x3(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    return inst;
}

bool rikka_ray_accepts(RikkaFrameRef frame, uint custom, uint geometry, uint hook, uint primitive, float2 barycentrics,
    bool shadow, uint channels) {
    RikkaTrace trace = frame.look.trace;
    uint index = rikka_ray_part(trace, custom, geometry);
    if (index == RIKKA_NONE) {
        return false;
    }
    RikkaInstance inst = rikka_ray_instance(frame, index, hook);
    if (shadow && ((inst.flags & RIKKA_CASTS_SHADOW) == 0u || (inst.light_channels & channels) == 0u)) {
        return false;
    }
    if ((inst.flags & (RIKKA_MASKED | RIKKA_TRANSPARENT)) == 0u) {
        return true;
    }
    RikkaMaterial material = frame.materials[inst.material];
    RikkaRayVertex v = rikka_ray_vertex(inst, rikka_ray_triangle(trace, inst, primitive), barycentrics);
    float cutoff = (inst.flags & RIKKA_MASKED) != 0u ? material.alpha_cutoff : RIKKA_RAY_CUTOFF;
    return rikka_ray_albedo(frame, material, v, 0.0).a >= cutoff;
}

float3 rikka_ray_offset(float3 p, float3 n) {
    const float origin = 1.0 / 32.0;
    const float float_scale = 1.0 / 65536.0;
    const float int_scale = 256.0;
    int3 of_i = int3(int_scale * n);
    float3 p_i = float3(
        asfloat(asint(p.x) + ((p.x < 0.0) ? -of_i.x : of_i.x)),
        asfloat(asint(p.y) + ((p.y < 0.0) ? -of_i.y : of_i.y)),
        asfloat(asint(p.z) + ((p.z < 0.0) ? -of_i.z : of_i.z)));
    return float3(
        abs(p.x) < origin ? p.x + float_scale * n.x : p_i.x,
        abs(p.y) < origin ? p.y + float_scale * n.y : p_i.y,
        abs(p.z) < origin ? p.z + float_scale * n.z : p_i.z);
}

float3 rikka_ray_lift(float3 position, float3 normal, float depth) {
    return rikka_ray_offset(position + normal * (1e-3 + 1e-3 * depth), normal);
}

float rikka_ray_escapes(RikkaFrameRef frame, float3 origin, float3 direction, float reach, bool shadow, uint channels) {
    RikkaTrace trace = frame.look.trace;
    RayDesc ray;
    ray.Origin = origin;
    ray.Direction = direction;
    ray.TMin = 0.0;
    ray.TMax = reach;
    RayQuery<RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH | RAY_FLAG_SKIP_PROCEDURAL_PRIMITIVES> query;
    uint mask = shadow ? RIKKA_RAY_SHADOW : RIKKA_RAY_SEEN;
    query.TraceRayInline(rikka_ray_scene(rikka_address(trace.scene)), RAY_FLAG_NONE, mask, ray);
    while (query.Proceed()) {
        if (query.CandidateType() == CANDIDATE_NON_OPAQUE_TRIANGLE
            && rikka_ray_accepts(frame, query.CandidateInstanceID(), query.CandidateGeometryIndex(),
                query.CandidateInstanceContributionToHitGroupIndex(), query.CandidatePrimitiveIndex(),
                query.CandidateTriangleBarycentrics(), shadow, channels)) {
            query.CommitNonOpaqueTriangleHit();
        }
    }
    return query.CommittedStatus() == COMMITTED_TRIANGLE_HIT ? 0.0 : 1.0;
}

float rikka_ray_visible(RikkaFrameRef frame, float3 origin, float3 direction, float reach, uint channels) {
    return rikka_ray_escapes(frame, origin, direction, reach, true, channels);
}

struct RikkaRayHit {
    bool hit;
    float distance;
    uint instance;
    uint channels;
    uint figure;
    float3 position;
    float3 normal;
    float4 albedo;
    float3 emission;
};

RikkaRayHit rikka_ray_closest(RikkaFrameRef frame, float3 origin, float3 direction, float reach) {
    RikkaTrace trace = frame.look.trace;
    RayDesc ray;
    ray.Origin = origin;
    ray.Direction = direction;
    ray.TMin = 0.0;
    ray.TMax = reach;
    RayQuery<RAY_FLAG_SKIP_PROCEDURAL_PRIMITIVES> query;
    query.TraceRayInline(rikka_ray_scene(rikka_address(trace.scene)), RAY_FLAG_NONE, RIKKA_RAY_SEEN, ray);
    while (query.Proceed()) {
        if (query.CandidateType() == CANDIDATE_NON_OPAQUE_TRIANGLE
            && rikka_ray_accepts(frame, query.CandidateInstanceID(), query.CandidateGeometryIndex(),
                query.CandidateInstanceContributionToHitGroupIndex(), query.CandidatePrimitiveIndex(),
                query.CandidateTriangleBarycentrics(), false, RIKKA_ALL_CHANNELS)) {
            query.CommitNonOpaqueTriangleHit();
        }
    }
    RikkaRayHit hit;
    hit.hit = query.CommittedStatus() == COMMITTED_TRIANGLE_HIT;
    hit.distance = reach;
    hit.instance = RIKKA_NONE;
    hit.channels = 0u;
    hit.figure = 0u;
    hit.position = origin + direction * reach;
    hit.normal = -direction;
    hit.albedo = float4(0.0);
    if (!hit.hit) {
        return hit;
    }
    hit.distance = query.CommittedRayT();
    hit.instance = rikka_ray_part(trace, query.CommittedInstanceID(), query.CommittedGeometryIndex());
    RikkaInstance inst = rikka_ray_instance(frame, hit.instance, query.CommittedInstanceContributionToHitGroupIndex());
    hit.channels = inst.light_channels;
    hit.figure = inst.figure;
    RikkaMaterial material = frame.materials[inst.material];
    RikkaRayVertex v = rikka_ray_vertex(inst, rikka_ray_triangle(trace, inst, query.CommittedPrimitiveIndex()),
        query.CommittedTriangleBarycentrics());
    hit.position = origin + direction * hit.distance;
    hit.normal = dot(v.normal, direction) > 0.0 ? -v.normal : v.normal;
    hit.albedo = rikka_ray_response(frame, material, v, RIKKA_RAY_TEXTURE_LEVEL);
    hit.emission = rikka_ray_emission(frame, material, v);
    return hit;
}

bool rikka_ray_lights(RikkaLight light, uint channels, uint figure) {
    return (light.channels & channels) != 0u && (light.figure == 0u || light.figure == figure);
}

float3 rikka_ray_direct(RikkaFrameRef frame, float3 position, float3 normal, uint channels, uint figure, float pick) {
    RikkaViewRef view = frame.view;
    float3 origin = rikka_ray_offset(position, normal);
    float3 sum = float3(0.0);
    for (uint i = 0u; i < view.directional_count; ++i) {
        RikkaLight light = frame.lights[i];
        float3 l = -light.direction;
        float n_dot_l = dot(normal, l);
        if (n_dot_l <= 0.0 || !rikka_ray_lights(light, channels, figure)) {
            continue;
        }
        float visible = light.shadow == RIKKA_NONE ? 1.0 : rikka_ray_visible(frame, origin, l, 1e4, light.channels);
        sum += light.color * light.intensity * n_dot_l * visible;
    }
    uint locals = view.light_count - view.directional_count;
    if (locals > 0u) {
        uint index = view.directional_count + min(uint(pick * float(locals)), locals - 1u);
        RikkaLight light = frame.lights[index];
        float3 l;
        float attenuation;
        rikka_light_sample(frame, light, position, l, attenuation);
        float n_dot_l = dot(normal, l);
        if (n_dot_l > 0.0 && attenuation > 0.0 && rikka_ray_lights(light, channels, figure)) {
            float distance = length(light.position - position);
            float visible = light.shadow == RIKKA_NONE ? 1.0 : rikka_ray_visible(frame, origin, l, distance, light.channels);
            sum += light.color * light.intensity * attenuation * n_dot_l * visible * float(locals);
        }
    }
    return sum * (1.0 / RIKKA_PI);
}

float3 rikka_ray_around(float3 normal, float3 local) {
    float sign = normal.z >= 0.0 ? 1.0 : -1.0;
    float a = -1.0 / (sign + normal.z);
    float b = normal.x * normal.y * a;
    float3 tangent = float3(1.0 + sign * normal.x * normal.x * a, sign * b, -sign * normal.x);
    float3 bitangent = float3(b, sign + normal.y * normal.y * a, -normal.y);
    return tangent * local.x + bitangent * local.y + normal * local.z;
}

float3 rikka_ray_cosine(float3 normal, float2 noise) {
    float radius = sqrt(noise.x);
    float phi = 2.0 * RIKKA_PI * noise.y;
    float3 local = float3(radius * cos(phi), radius * sin(phi), sqrt(max(1.0 - noise.x, 0.0)));
    return normalize(rikka_ray_around(normal, local));
}

float3 rikka_ray_cone(float3 direction, float cos_max, float2 noise) {
    float cos_theta = lerp(1.0, cos_max, noise.x);
    float sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
    float phi = 2.0 * RIKKA_PI * noise.y;
    return normalize(rikka_ray_around(direction, float3(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta)));
}

float2 rikka_ray_noise(uint2 pixel, uint frame, uint dimension) {
    uint seed = rikka_pcg_hash(pixel.x + rikka_pcg_hash(pixel.y + rikka_pcg_hash(frame * 8u + dimension)));
    float2 base = float2(rikka_interleaved_gradient(float2(pixel) + float2(float(dimension) * 7.0, 0.0)),
                         float(seed >> 8u) * (1.0 / 16777216.0));
    const float2 golden = float2(0.7548776662, 0.5698402910);
    return fract(base + golden * float(frame % 64u));
}

static const float RIKKA_RAY_LIT_FLOOR = 1e-4;

float rikka_ray_shadow(RikkaFrameRef frame, RikkaLight light, float3 position, float3 normal, float depth, uint2 pixel,
    uint channel, uint samples) {
    uint frame_index = frame.look.trace.frame;
    float3 origin = rikka_ray_lift(position, normal, depth);
    float visible = 0.0;
    for (uint s = 0u; s < samples; ++s) {
        float2 noise = rikka_ray_noise(pixel, frame_index * samples + s, channel);
        float3 direction;
        float reach;
        if (light.kind == RIKKA_LIGHT_DIRECTIONAL) {
            direction = rikka_ray_cone(-light.direction, cos(light.size), noise);
            reach = 1e4;
        } else {
            float3 offset = light.position - position;
            float distance = max(length(offset), 1e-4);
            float cos_max = distance * rsqrt(distance * distance + light.size * light.size);
            direction = rikka_ray_cone(offset / distance, cos_max, noise);
            reach = max(distance - light.size, 0.0);
        }
        visible += rikka_ray_visible(frame, origin, direction, reach, light.channels);
    }
    return visible / float(max(samples, 1u));
}

float4 rikka_ray_shadows(RikkaFrameRef frame, uint4 lights, uint samples, float3 position, float3 normal, float depth,
    uint2 pixel) {
    float4 visibility = float4(1.0);
    [unroll]
    for (uint channel = 0u; channel < RIKKA_TRACED_LIGHTS; ++channel) {
        uint index = lights[channel];
        if (index == RIKKA_NONE) {
            continue;
        }
        RikkaLight light = frame.lights[index];
        float3 l;
        float attenuation;
        rikka_light_sample(frame, light, position, l, attenuation);
        if (attenuation <= RIKKA_RAY_LIT_FLOOR) {
            continue;
        }
        float3 facing = dot(normal, l) >= 0.0 ? normal : -normal;
        visibility[channel] = rikka_ray_shadow(frame, light, position, facing, depth, pixel, channel, samples);
    }
    return visibility;
}

float4 rikka_ray_traced_shadows(RikkaFrameRef frame, float3 position, float3 normal, float depth, uint2 pixel) {
    RikkaTrace trace = frame.look.trace;
    uint4 lights = uint4(trace.lights[0], trace.lights[1], trace.lights[2], trace.lights[3]);
    return rikka_ray_shadows(frame, lights, trace.shadow_samples, position, normal, depth, pixel);
}

float3 rikka_ray_sky_bounce(RikkaFrameRef frame, float3 position, float3 normal, float2 noise, float reach) {
    float3 direction = rikka_ray_cosine(normal, noise);
    float open = rikka_ray_escapes(frame, rikka_ray_offset(position, normal), direction, reach, false, RIKKA_ALL_CHANNELS);
    return rikka_sky_specular(frame, direction, 1.0) * open;
}

float3 rikka_ray_shade(RikkaFrameRef frame, RikkaRayHit hit, float pick, float2 noise, float reach) {
    float3 incident = rikka_ray_direct(frame, hit.position, hit.normal, hit.channels, hit.figure, pick)
        + rikka_ray_sky_bounce(frame, hit.position, hit.normal, noise, reach);
    return hit.emission + hit.albedo.rgb * incident;
}
