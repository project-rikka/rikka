import canvas.instance;
import canvas.sdf;
import canvas.glyph_curves;
import canvas.color;

struct GroupClip {
    float4 clip;
    float4 clip_radii;
}

[[vk::binding(1, 0)]] StructuredBuffer<float2> group_offsets;
[[vk::binding(2, 0)]] StructuredBuffer<uint> shown_bits;
[[vk::binding(3, 0)]] StructuredBuffer<GroupClip> group_clips;
[[vk::binding(5, 0)]] StructuredBuffer<float2> node_moves;
[[vk::binding(6, 0)]] StructuredBuffer<Conversion> conversions;
[[vk::binding(7, 0)]] StructuredBuffer<GroupClip> node_clips;

[[vk::binding(0, 1)]] Sampler2D textures[];

static const float4 UNCLIPPED = float4(0.0, 0.0, -1.0, -1.0);
static const float TURNED_EDGE = 1.0;

float4 rounding_within(float4 shape, float4 radii, float2 from, float2 to) {
    float2 near = from - shape.xy;
    float2 far = shape.xy + shape.zw - to;
    float4 inset = float4(max(near.x, near.y), max(far.x, near.y), max(far.x, far.y), max(near.x, far.y));
    return max(radii - inset, float4(0.0));
}

struct BoundClip {
    float4 clip;
    float4 radii;
    bool culled;
}

BoundClip bound_clip(Instance inst, uint flags, float2 shift) {
    bool own = (flags & FLAG_CLIP) != 0u;
    if ((flags & FLAG_OUTSIDE_GROUP_CLIP) != 0u) {
        if (!own) {
            return { UNCLIPPED, float4(0.0), false };
        }
        GroupClip mine = node_clips[inst.clip_index];
        return { mine.clip, mine.clip_radii, false };
    }
    GroupClip held = group_clips[inst.group];
    if (held.clip.z < 0.0) {
        if (!own) {
            return { UNCLIPPED, float4(0.0), false };
        }
        GroupClip mine = node_clips[inst.clip_index];
        return { mine.clip, mine.clip_radii, false };
    }
    float4 outer = float4(held.clip.xy - shift, held.clip.zw);
    if (!own) {
        return { outer, held.clip_radii, false };
    }
    GroupClip mine = node_clips[inst.clip_index];
    float2 from = max(mine.clip.xy, outer.xy);
    float2 to = min(mine.clip.xy + mine.clip.zw, outer.xy + outer.zw);
    if (from.x >= to.x || from.y >= to.y) {
        return { UNCLIPPED, float4(0.0), true };
    }
    float4 clip = float4(from, to - from);
    float4 rounded = max(rounding_within(mine.clip, mine.clip_radii, from, to),
                         rounding_within(outer, held.clip_radii, from, to));
    return { clip, min(rounded, float4(min(clip.z, clip.w) * 0.5)), false };
}
[shader("vertex")]
Varyings vertex(int vertex_index : SV_VulkanVertexID, int instance_index : SV_VulkanInstanceID) {
    Varyings out;
    Instance inst = instances[instance_index];
    if ((shown_bits[inst.shown >> 5] & (1u << (inst.shown & 31u))) == 0u) {
        out.position = float4(2.0, 2.0, 0.0, 1.0);
        return out;
    }
    uint kind = inst.meta & KIND_MASK;
    uint flags = (inst.meta >> FLAGS_SHIFT) & FLAGS_MASK;
    uint color_slot = inst.meta >> COLOR_SHIFT;

    float2 group_offset = group_offsets[inst.group];
    float2 clip_shift = (inst.clip_group == inst.group) ? group_offset : group_offsets[inst.clip_group];
    BoundClip bound = bound_clip(inst, flags, clip_shift);
    if (bound.culled) {
        out.position = float4(2.0, 2.0, 0.0, 1.0);
        return out;
    }

    float2 corner = float2(float(vertex_index >> 1), float(vertex_index & 1));
    float2 shift = group_offset + node_moves[inst.shown];
    float2 pos = inst.bounds.xy + shift + corner * inst.bounds.zw;
    float2 spot = (corner - 0.5) * inst.bounds.zw;
    if ((flags & FLAG_TURNED) != 0u) {
        spot = (corner - 0.5) * (inst.bounds.zw + 2.0 * TURNED_EDGE);
        corner = spot / max(inst.bounds.zw, float2(1.0e-6)) + 0.5;
        float2 center = inst.bounds.xy + shift + inst.bounds.zw * 0.5;
        pos = center + float2(inst.turn.x * spot.x - inst.turn.y * spot.y,
                              inst.turn.y * spot.x + inst.turn.x * spot.y);
    }
    float2 local = ((flags & FLAG_UV_SWAP) != 0u) ? corner.yx : corner;

    bool glass = kind == KIND_BACKDROP;
    float4 uv = glass ? pc.glass_uv : inst.uv;
    uint tex = glass ? pc.glass_tex : inst.tex;

    Conversion ui = conversions[0];
    uint slot = kind == KIND_TEXTURED ? color_slot : 0u;
    uint decode_mode = (slot == 0u) ? ui.mode.x : conversions[slot].mode.x;
    uint packed = (inst.meta & 0xFFFFu) | ((decode_mode & 0xFFu) << 16u) | ((ui.mode.w & 0xFFu) << 24u);

    out.pos = float4(pos, spot);
    out.uv = lerp(uv.xy, uv.zw, local);
    out.what = uint4(packed, tex, slot, inst.cut_index);
    out.shape = float4(inst.bounds.zw * 0.5, inst.stroke_width, inst.opacity);
    out.radii = inst.radii;
    out.fill = convert(inst.fill, ui);
    out.stroke = convert(inst.stroke, ui);
    out.clip_shift = clip_shift;
    if (bound.clip.z >= 0.0) {
        float2 half_clip = bound.clip.zw * 0.5;
        out.clip = float4(bound.clip.xy + half_clip + clip_shift, half_clip);
        out.clip_radii = bound.radii;
    } else {
        out.clip = UNCLIPPED;
        out.clip_radii = float4(0.0);
    }

    float2 placed = pos / pc.target.xy;
    if (pc.target[2] > 0.5) {
        placed = placed.yx;
    }
    uint flips = uint(pc.target[3]);
    placed = lerp(placed, 1.0 - placed, float2(bool2((flips & 1u) != 0u, (flips & 2u) != 0u)));
    out.position = float4(placed * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

float sd_ellipse(float2 p, float2 r) {
    float2 safe_r = max(r, float2(1.0e-6));
    float k = length(p / safe_r);
    return (k - 1.0) * min(safe_r.x, safe_r.y);
}

float sd_arc(float2 p, float4 angles, float ra, float rb) {
    float2 mid = angles.xy;

    float2 q = float2(dot(p, mid), abs(mid.x * p.y - mid.y * p.x));
    float d = (angles.z * q.x >= angles.w * q.y)
        ? abs(length(q) - ra)
        : length(q - angles.wz * ra);
    return d - rb;
}

float luminance(float3 c) {
    return dot(c, float3(0.2126, 0.7152, 0.0722));
}

float dither(float2 p) {
    int2 c = int2(p) & 3;
    int a = c.x ^ c.y;
    int index = ((a & 1) << 3) | ((c.y & 1) << 2) | (a & 2) | ((c.y & 2) >> 1);
    return float(index) / 16.0;
}

float4 converted(float4 sampled, uint decode_mode, uint color_slot) {
    if (decode_mode == DECODE_IDENTITY) {
        return sampled;
    }
    return convert(sampled, conversions[color_slot]);
}

float coverage(float sdf) {
    return clamp(0.5 - sdf, 0.0, 1.0);
}

[shader("fragment")]
float4 fragment(Varyings in) : SV_Target {
    uint packed = in.what.x;
    uint kind = packed & KIND_MASK;
    uint flags = (packed >> FLAGS_SHIFT) & FLAGS_MASK;
    uint decode_mode = (packed >> 16u) & 0xFFu;
    uint encode_mode = packed >> 24u;
    uint tex = in.what.y;
    uint color_slot = in.what.z;
    float2 pos = in.pos.xy;
    float2 p = in.pos.zw;
    float2 half_size = in.shape.xy;
    float stroke_width = in.shape.z;
    float opacity = in.shape.w;

    float clip_cov = 1.0;
    if (in.clip.z >= 0.0) {
        clip_cov = coverage(sd_rounded_rect(pos - in.clip.xy, in.clip.zw, in.clip_radii));
    }
    if ((flags & FLAG_CUT) != 0u) {
        GroupClip cut = node_clips[in.what.w];
        float2 clipped = pos - in.clip_shift;
        clip_cov *= 1.0 - coverage(sd_rounded_rect(clipped - cut.clip.xy - cut.clip.zw * 0.5,
                                                   cut.clip.zw * 0.5, cut.clip_radii));
    }

    bool mipmapped = (kind == KIND_TEXTURED) && ((flags & FLAG_PIXELATED) == 0u);
    if (!mipmapped && clip_cov <= 0.0) {
        discard;
    }

    if (kind == KIND_GLYPH_MASK) {
        float mask = textures[NonUniformResourceIndex(tex)].SampleLevel(in.uv, 0.0).r;
        return in.fill * mask * clip_cov;
    }
    if (kind == KIND_GLYPH_CURVES) {
        return in.fill * glyph_curve_coverage(tex, in.uv, in.radii.x) * clip_cov;
    }
    if (kind == KIND_GLYPH_COLOR) {
        float4 s = converted(textures[NonUniformResourceIndex(tex)].SampleLevel(in.uv, 0.0),
                             decode_mode, color_slot);
        return s * in.fill * clip_cov;
    }

    if ((flags & FLAG_ARC) != 0u) {
        float r = min(half_size.x, half_size.y);
        float rb = min(stroke_width * 0.5, r);
        return in.stroke * coverage(sd_arc(p, in.radii, r - rb, rb)) * clip_cov;
    }

    float sdf = ((flags & FLAG_CIRCLE) != 0u)
        ? sd_ellipse(p, half_size)
        : sd_rounded_rect(p, half_size, in.radii);

    float4 base;
    if (kind == KIND_TEXTURED) {
        float4 s;
        if ((flags & FLAG_PIXELATED) != 0u) {
            uint width;
            uint height;
            textures[NonUniformResourceIndex(tex)].GetDimensions(width, height);
            int2 size = int2(int(width), int(height));
            int2 texel = clamp(int2(floor(in.uv * float2(size))), int2(0), size - 1);
            s = textures[NonUniformResourceIndex(tex)].Load(int3(texel, 0));
        } else {
            s = textures[NonUniformResourceIndex(tex)].Sample(in.uv);
            if (clip_cov <= 0.0) {
                discard;
            }
        }

        base = converted(s, decode_mode, color_slot) * in.fill;
    } else if (kind == KIND_BACKDROP && tex != NO_GLASS) {
        float4 tint = in.fill;
        float2 at = clamp(in.uv, pc.glass_bounds.xy, pc.glass_bounds.zw);
        float4 s = textures[pc.glass_tex].SampleLevel(at, 0.0);

        if ((flags & FLAG_LENS) != 0u) {
            if (pc.adapt > 0.0 && tint.a > 1.0e-4) {
                float inv_a = 1.0 / tint.a;
                float clash = clamp(
                    abs(luminance(s.rgb) - luminance(tint.rgb) * inv_a) * 2.0 - 0.35,
                    0.0, 1.0);
                tint *= min(1.0 + pc.adapt * clash, opacity * inv_a);
            }
        }

        base = tint + s * max(opacity - tint.a, 0.0);

        if (encode_mode != ENCODE_LINEAR) {
            base.rgb += (dither(pos) - 0.5) * base.a * (1.0 / 255.0);
        }
    } else {
        base = in.fill;
    }

    float outer = coverage(sdf);
    float inner = (stroke_width > 0.0) ? coverage(sdf + stroke_width) : outer;

    return (base * inner + in.stroke * (outer - inner)) * clip_cov;
}
