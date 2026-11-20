__exported import rikka.material.context;

struct RikkaSurfaceData<D> {
    float3 albedo;
    float alpha;
    float3 normal;
    float occlusion;
    float3 emission;
    D model;
};

RikkaSurfaceData<D> rikka_surface_data<D>(RikkaPixel pixel, D model) {
    RikkaSurfaceData<D> s;
    s.albedo = float3(1.0);
    s.alpha = 1.0;
    s.normal = pixel.geometric_normal;
    s.occlusion = 1.0;
    s.emission = float3(0.0);
    s.model = model;
    return s;
}

interface ISurface<D> {
    [constref]
    RikkaSurfaceData<D> evaluate(RikkaPixel pixel, RikkaContext context);

    [constref]
    float coverage(RikkaCoverage coverage, RikkaContext context) {
        return 1.0;
    }

    [constref]
    float3 transmittance(RikkaCoverage at, RikkaContext context) {
        return float3(1.0 - this.coverage(at, context));
    }

    [constref]
    RikkaHull hull(RikkaHullVertex vertex, RikkaContext context, float scale) {
        return rikka_no_hull(vertex);
    }
};

struct RikkaVertex {
    float3 local_position;
    float3 world_position;
    float3 normal;
    float4 tangent;
    float2 uv;
    float4 color;
    uint vertex_index;
};

interface IVertex {
    [constref]
    void deform(inout RikkaVertex vertex, RikkaContext context, float time);
};

RikkaVertex rikka_vertex(RikkaInstance inst, uint vertex_index, float3 local, float4 world) {
    RikkaAttribute packed = ((RikkaAttributeRef)inst.attributes)[vertex_index];
    float4 tangent = rikka_decode_tangent(packed.tangent);
    RikkaVertex v;
    v.local_position = local;
    v.world_position = world.xyz;
    v.normal = mul(inst.normal_matrix, rikka_decode_octahedral(packed.normal));
    v.tangent = float4(mul((float3x3)inst.transform, tangent.xyz), tangent.w);
    v.uv = rikka_decode_uv(packed.uv);
    v.color = rikka_decode_color(packed.color);
    v.vertex_index = vertex_index;
    return v;
}

void rikka_deform<S>(__constref S surface, inout RikkaVertex vertex, RikkaContext context, float time)
    where optional S : IVertex
{
    if (S is IVertex) {
        surface.deform(vertex, context, time);
    }
}

RikkaVertex rikka_hooked_vertex<S>(__constref S surface, RikkaContext context, uint vertex_index)
    where optional S : IVertex
{
    RikkaInstance inst = context.instance;
    float3 local = ((RikkaPositionRef)inst.positions)[vertex_index];
    float4 world = mul(inst.transform, float4(local, 1.0));
    RikkaVertex v = rikka_vertex(inst, vertex_index, local, world);
    rikka_deform<S>(surface, v, context, context.time);
    return v;
}

RikkaAttribute rikka_encode_attribute(RikkaVertex v) {
    RikkaAttribute packed;
    packed.normal = rikka_encode_octahedral(v.normal);
    packed.tangent = rikka_encode_tangent(v.tangent);
    packed.uv = packHalf2x16(v.uv);
    packed.color = packUnorm4x8(v.color);
    return packed;
}
