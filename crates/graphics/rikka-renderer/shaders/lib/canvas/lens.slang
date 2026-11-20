import canvas.fx;
import canvas.sdf;

static const float LENS_FAR = 16384.0;

float2 lens_half_size() {
    return fx.params[0].zw * 0.5;
}

bool lens_joined() {
    return fx.params[4][0] > 0.0;
}

int lens_side() {
    return int(fx.params[3][1] + 0.5);
}

float2 lens_canon(float2 v) {
    int side = lens_side();
    if (side == 1) {
        return float2(v.x, -v.y);
    }
    if (side == 2) {
        return v.yx;
    }
    if (side == 3) {
        return float2(v.y, -v.x);
    }
    return v;
}

float2 lens_world(float2 v) {
    int side = lens_side();
    if (side == 1) {
        return float2(v.x, -v.y);
    }
    if (side == 2) {
        return v.yx;
    }
    if (side == 3) {
        return float2(-v.y, v.x);
    }
    return v;
}

float2 lens_point(float2 uv, bool joined) {
    float2 at = uv / fx.src_texel * fx.src_scale + fx.src_origin;
    if (joined) {
        at = lens_canon(at);
    }
    return at - fx.params[0].xy - lens_half_size();
}

float lens_edge(float2 p) {
    return sd_rounded_rect(p, lens_half_size(), fx.params[1]);
}

float lens_bevel() {
    float2 half_size = lens_half_size();
    return min(fx.params[2][0], min(half_size.x, half_size.y));
}

float lens_seam() {
    return lens_half_size().y;
}

float2 lens_span() {
    float centre = fx.params[0][0] + lens_half_size().x;
    return fx.params[3].zw - centre;
}

float2 lens_joint() {
    return lens_span() + float2(-fx.params[4][1], fx.params[4][2]);
}

float lens_depth(float2 p) {
    float2 half_size = lens_half_size();
    float seam = lens_seam();
    float2 span = lens_span();
    float2 joint = lens_joint();
    float left = fx.params[4][1];
    float right = fx.params[4][2];
    float bottom = seam + fx.params[4][0];

    float4 top_radii = float4(fx.params[1].xy, 0.0, 0.0);
    float host_limit = min(half_size.x, half_size.y);
    float depth = max(-sd_rounded_rect_within(p - float2(0.0, LENS_FAR),
                                              half_size + float2(0.0, LENS_FAR), top_radii,
                                              host_limit), 0.0);
    depth = min(depth, length(max(float2(p.x - joint.x, seam - p.y), float2(0.0))));
    depth = min(depth, length(max(float2(joint.y - p.x, seam - p.y), float2(0.0))));
    if (left > 0.0) {
        depth = min(depth, max(length(p - float2(joint.x, seam + left)) - left, 0.0));
    }
    if (right > 0.0) {
        depth = min(depth, max(length(p - float2(joint.y, seam + right)) - right, 0.0));
    }
    depth = min(depth, length(max(float2(p.x - span.x, seam + left - p.y), float2(0.0))));
    depth = min(depth, length(max(float2(span.y - p.x, seam + right - p.y), float2(0.0))));
    if (p.y >= seam + max(left, right)) {
        float2 centre = float2((span.x + span.y) * 0.5, bottom - LENS_FAR);
        float2 limb = float2((span.y - span.x) * 0.5, LENS_FAR);
        float limb_limit = min(limb.x, fx.params[4][0] * 0.5);
        depth = min(depth, max(-sd_rounded_rect_within(p - centre, limb,
                                                       float4(0.0, 0.0, right, left),
                                                       limb_limit), 0.0));
    } else {
        depth = min(depth, max(bottom - p.y, 0.0));
    }
    return depth;
}

float2 lens_body_normal(float2 p, float depth) {
    float weight = 0.0;
    float seam = lens_seam();
    if (p.y < seam) {
        float2 joint = lens_joint();
        float bevel = lens_bevel();
        float apart = length(float2(max(max(joint.x - p.x, p.x - joint.y), 0.0), seam - p.y));
        weight = smoothstep(bevel, 2.0 * bevel, apart);
        if (weight >= 1.0) {
            return bevel_normal(p, lens_half_size());
        }
    }
    float2 slope = float2(lens_depth(p + float2(1.0, 0.0)), lens_depth(p + float2(0.0, 1.0))) - depth;
    float steep = length(slope);
    if (steep >= 1.0e-4 && weight <= 0.0) {
        return -slope / steep;
    }
    float2 own = bevel_normal(p, lens_half_size());
    if (steep < 1.0e-4) {
        return own;
    }
    float2 normal = lerp(-slope / steep, own, weight);
    float len = length(normal);
    return (len > 1.0e-6) ? normal / len : own;
}

float2 lens_swell(float2 p) {
    return -p * fx.params[3][0];
}

float lens_displacement(float edge) {
    float bevel = lens_bevel();
    float t = (bevel > 0.0) ? -edge / bevel : 1.0;
    if (t >= 1.0) {
        return 0.0;
    }
    float4 optics = fx.params[2];
    float depth = optics.y * bevel / max(optics.x, 1.0e-6);
    return bevel_displacement(t, optics.z) * depth;
}

float4 lens_plain(float2 uv) {
    return fx_sample_level(fx_clamp_src(uv), 0.0);
}

float4 lens_sample(float2 uv, float2 bend) {
    float2 offset = bend * (fx.src_texel / fx.src_scale);
    float split = fx.params[2][3];
    float4 centre = lens_plain(uv + offset);
    if (split <= 0.0) {
        return centre;
    }

    return float4(
        lens_plain(uv + offset * (1.0 - split)).r,
        centre.g,
        lens_plain(uv + offset * (1.0 + split)).b,
        centre.a);
}

float4 lens_through(float2 uv) {
    bool joined = lens_joined();
    float2 p = lens_point(uv, joined);
    if (joined) {
        float depth = lens_depth(p);
        if (depth <= 0.0) {
            return lens_plain(uv);
        }
        float2 offset = lens_swell(p);
        float displacement = lens_displacement(-depth);
        if (displacement > 0.0) {
            offset += lens_body_normal(p, depth) * displacement;
        }
        return lens_sample(uv, lens_world(offset));
    }
    float edge = lens_edge(p);
    if (edge > 0.0) {
        return lens_plain(uv);
    }
    float2 offset = lens_swell(p);
    float displacement = lens_displacement(edge);
    if (displacement > 0.0) {
        offset += bevel_normal(p, lens_half_size()) * displacement;
    }
    return lens_sample(uv, offset);
}
