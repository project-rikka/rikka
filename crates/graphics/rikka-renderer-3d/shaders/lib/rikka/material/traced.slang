import rikka.material.context;
import rikka.traced;
import rikka.ray;

static const float RIKKA_TRACED_INWARD = 1.5;

bool rikka_traced_wanted(RikkaContext context) {
    return (context.instance.flags & RIKKA_RECEIVES_SHADOW) != 0u && rikka_traced_available(context.frame);
}

uint rikka_traced(RikkaContext context, RikkaPixel pixel) {
    if (!rikka_traced_wanted(context)) {
        return RIKKA_UNTRACED;
    }
    RikkaFrameRef frame = context.frame;
    int2 own = rikka_traced_pixel_at(context.view, pixel.frag_coord.xy);
    if (!rikka_facial(context) && rikka_traced_shows(frame, own, pixel.view_depth)) {
        return rikka_traced_pack(rikka_traced_image(frame, own));
    }
    return rikka_traced_pack(rikka_ray_traced_shadows(
        frame, rikka_shadow_receiver(context, pixel.world_position), pixel.geometric_normal, pixel.view_depth, uint2(own)));
}

uint rikka_traced_hull(RikkaContext context, RikkaPixel pixel, float3 surface_position) {
    if (!rikka_traced_wanted(context)) {
        return RIKKA_UNTRACED;
    }
    RikkaFrameRef frame = context.frame;
    RikkaViewRef view = context.view;
    float depth = dot(surface_position - view.camera_position, view.camera_forward);
    float2 at;
    if (!rikka_facial(context) && rikka_traced_projects(view, surface_position, at)) {
        int2 source = rikka_traced_pixel_at(view, at);
        if (rikka_traced_shows(frame, source, depth)) {
            return rikka_traced_pack(rikka_traced_image(frame, source));
        }
        float2 inward = at - pixel.frag_coord.xy;
        if (dot(inward, inward) > 0.0) {
            int2 inner = rikka_traced_pixel_at(view, at + normalize(inward) * RIKKA_TRACED_INWARD);
            if (rikka_traced_shows(frame, inner, depth)) {
                return rikka_traced_pack(rikka_traced_image(frame, inner));
            }
        }
    }
    return rikka_traced_pack(rikka_ray_traced_shadows(
        frame, rikka_shadow_receiver(context, surface_position), pixel.geometric_normal, depth, uint2(pixel.frag_coord.xy)));
}
