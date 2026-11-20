__exported import rikka.material.surface;
__exported import rikka.shading;

struct RikkaShadowQuery {
    RikkaFrameRef frame;
    RikkaLight light;
    RikkaShadowPoint point;
    float contact;
    bool leading;
    bool known;
    RikkaShadowSample result;

    [mutating]
    RikkaShadowSample sample() {
        if (!known) {
            result = rikka_shadow_evaluate(frame, light, point);
            known = true;
        }
        return result;
    }

    [mutating]
    float map() {
        return sample().visibility;
    }

    float screen() {
        return leading ? contact : 1.0;
    }

    [mutating]
    float visibility() {
        return map() * screen();
    }

    [mutating]
    float3 transmittance() {
        return sample().tint * visibility();
    }

    float hard() {
        return rikka_shadow_hard(frame, light, point);
    }
};

struct RikkaIndirect {
    RikkaFrameRef frame;
    float2 occlusion;
    float3 bend;
    float3 ambient;
    bool looked;
    float3 bounce;
    bool bounced;

    float3 irradiance(float3 normal) {
        return rikka_sky_irradiance(frame, normal);
    }

    float3 bent(float3 normal) {
        float3 direction = occlusion.x * normal + bend;
        float length_squared = dot(direction, direction);
        return any(bend != float3(0.0)) && length_squared > 1e-12 ? direction * rsqrt(length_squared) : normal;
    }

    float3 diffuse(float3 normal) {
        return bounced ? bounce : irradiance(bent(normal)) * occlusion.x;
    }

    float3 diffuse(float3 normal, float3 albedo, float material_occlusion) {
        if (bounced) {
            return bounce * rikka_multi_bounce(material_occlusion, albedo);
        }
        return irradiance(bent(normal)) * rikka_multi_bounce(material_occlusion * occlusion.x, albedo);
    }

    float3 multi_bounce(float3 albedo) {
        return rikka_multi_bounce(occlusion.x, albedo);
    }

    float specular_occlusion(float3 normal, float3 view, float roughness) {
        return specular_occlusion(normal, view, roughness, 1.0);
    }

    float specular_occlusion(float3 normal, float3 view, float roughness, float material_occlusion) {
        return rikka_bent_specular_occlusion(bent(normal), material_occlusion * occlusion.x, normal, view, roughness);
    }

    float3 specular(float3 direction, float roughness) {
        return rikka_sky_specular(frame, direction, roughness);
    }
};

struct RikkaGloss {
    [Default(1.0)] [SoftRange(0.0, 1.0)] [Group("reflections")] float roughness;
    [Default(0.0)] [SoftRange(0.0, 1.0)] [Group("reflections")] float reflection;
};

struct RikkaGBuffer {
    float roughness;
    float reflection;
    uint figure;
};

struct RikkaScatter {
    float4 diffuse;
    float4 profile;
};

RikkaScatter rikka_no_scatter() {
    RikkaScatter s;
    s.diffuse = float4(0.0);
    s.profile = float4(0.0);
    return s;
}

interface IScattering {
};

RikkaGBuffer rikka_gbuffer(RikkaContext context, RikkaGloss gloss) {
    RikkaGBuffer g;
    g.roughness = gloss.roughness;
    g.reflection = gloss.reflection;
    g.figure = context.instance.figure;
    return g;
}

struct RikkaSeeThrough {
    float amount;
    float reach;
};

struct RikkaSeeThroughTraits {
    [Default(0.0)] [Range(0.0, 1.0)] [Group("see through")] float amount;
    [Default(0.05)] [SoftRange(0.0, 0.5)] [Group("see through")] float reach;

    RikkaSeeThrough seen() {
        RikkaSeeThrough s;
        s.amount = amount;
        s.reach = reach;
        return s;
    }
};

interface IShadingModel {
    associatedtype Data;
    associatedtype State;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context);

    [constref]
    void direct(inout State state, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context);

    [constref]
    void transmit(inout State state, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
    }

    [constref]
    void indirect(inout State state, RikkaIndirect indirect, RikkaContext context);

    [constref]
    float4 resolve(State state, RikkaPixel pixel, RikkaContext context);

    [constref]
    float4 resolve_hull(State state, RikkaPixel pixel, RikkaContext context) {
        return resolve(state, pixel, context);
    }

    [constref]
    float4 shade(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        return rikka_lit(this, pixel, surface, context);
    }

    [constref]
    float3 emitted(RikkaSurfaceData<Data> surface, RikkaContext context) {
        return surface.emission;
    }

    [constref]
    float hull_scale(RikkaContext context) {
        return 1.0;
    }

    [constref]
    RikkaSeeThrough seen_through(RikkaContext context) {
        RikkaSeeThrough s;
        s.amount = 0.0;
        s.reach = 0.0;
        return s;
    }

    [constref]
    RikkaGBuffer gbuffer(RikkaContext context) {
        RikkaGBuffer g;
        g.roughness = 1.0;
        g.reflection = 0.0;
        g.figure = context.instance.figure;
        return g;
    }

    [constref]
    RikkaScatter scattered(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        return rikka_no_scatter();
    }
};

RikkaShadowQuery rikka_shadow_query(RikkaFrameRef frame, RikkaLight light, RikkaShadowPoint point, float contact, bool leading) {
    RikkaShadowQuery q;
    q.frame = frame;
    q.light = light;
    q.point = point;
    q.contact = contact;
    q.leading = leading;
    q.known = false;
    q.result = rikka_shadow_lit();
    return q;
}

void rikka_illuminate<M : IShadingModel>(__constref M model, inout M.State state, RikkaPixel pixel, RikkaContext context) {
    bool facial = rikka_facial(context);
    float screen = facial ? 1.0 : rikka_screen_shadow(context.frame, pixel.frag_coord.xy);
    float contact = (context.instance.flags & RIKKA_TRANSPARENT) != 0u ? 1.0 : screen;
    RikkaShadowPoint point;
    point.world_position = rikka_shadow_receiver(context, pixel.world_position);
    point.geometric_normal = pixel.geometric_normal;
    point.view_depth = pixel.view_depth;
    point.flags = context.instance.flags;
    point.rotation = rikka_shadow_rotation(context.frame, pixel.frag_coord.xy);
    point.traced = pixel.traced;

    RikkaLightIter it = rikka_lights_begin(
        context.frame, pixel.frag_coord.xy, pixel.view_depth, context.instance.figure, context.instance.light_channels);
    RikkaLight light;
    while (rikka_light_next(context.frame, it, light)) {
        if (!rikka_light_reaches(it, light)) {
            continue;
        }
        RikkaLightSample sample = rikka_sample_light(context.frame, light, pixel.world_position);
        RikkaShadowQuery shadow = rikka_shadow_query(context.frame, light, point, contact, sample.leading);
        model.direct(state, sample, shadow, context);
        if (!pixel.hull) {
            model.transmit(state, sample, shadow, context);
        }
    }

    float2 uv = pixel.frag_coord.xy * context.view.inverse_viewport;
    float4 ambient = facial ? float4(1.0, 0.0, 0.0, 0.0) : rikka_occlusion(context.frame, uv);
    bool owned = (context.instance.flags & RIKKA_DIRECT) == 0u && !pixel.hull;
    RikkaIndirect indirect;
    indirect.frame = context.frame;
    indirect.occlusion = float2(ambient.x, screen);
    indirect.bend = owned ? ambient.yzw : float3(0.0);
    indirect.ambient = context.view.ambient;
    indirect.looked = context.looked;
    indirect.bounce = rikka_traced_indirect(context.frame, uv, indirect.bounced);
    indirect.bounced = indirect.bounced && !facial;
    model.indirect(state, indirect, context);
}

M.State rikka_gather<M : IShadingModel>(__constref M model, RikkaPixel pixel, RikkaSurfaceData<M.Data> surface, RikkaContext context) {
    M.State state = model.prepare(pixel, surface, context);
    rikka_illuminate(model, state, pixel, context);
    return state;
}

float4 rikka_lit<M : IShadingModel>(__constref M model, RikkaPixel pixel, RikkaSurfaceData<M.Data> surface, RikkaContext context) {
    M.State state = rikka_gather(model, pixel, surface, context);
    if (pixel.hull) {
        return model.resolve_hull(state, pixel, context);
    }
    return model.resolve(state, pixel, context);
}
