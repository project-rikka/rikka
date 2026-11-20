import rikka.editor;

struct Push {
    RikkaFrameRef frame;
    uint axes;
    uint iteration;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint SHOW_GRID = 1u << 0u;
static const uint SHOW_AXES = 1u << 1u;
static const uint AXIS_X = 1u << 2u;
static const uint ALIGNED = 1u << 5u;
static const uint BEHIND_GEOMETRY = 1u << 6u;
static const uint PERSPECTIVE = 1u << 7u;

static const uint STEPS = 8u;
static const uint STEPS_DRAWN = 3u;
static const uint ITERATIONS = 4u;
static const float AXIS_EPSILON = 2e-7;
static const float Z_MIN_OFFSET = -0.01;
static const float Z_MAX_OFFSET = 0.0125;
static const float DASH_WIDTH = 4.0;
static const float FADE_START = 0.1;
static const float FRINGE = 1.0;
static const float4 CULLED = float4(0.0, 0.0, -2.0, 1.0);
static const float ADDITIVE_ALPHA[4] = { 1.0, 0.5, 0.25, 0.125 };

static const uint CORNER_END[6] = { 0u, 0u, 1u, 1u, 0u, 1u };
static const float CORNER_SIDE[6] = { -1.0, 1.0, -1.0, -1.0, 1.0, 1.0 };

struct GridVaryings {
    [[vk::location(0)]] noperspective float across;
    [[vk::location(1)]] noperspective float along;
    [[vk::location(2)]] float2 coord;
    [[vk::location(3)]] float3 world;
    [[vk::location(4)]] nointerpolation float4 color;
    [[vk::location(5)]] nointerpolation uint on_floor;
};

struct LineData {
    float2 p;
    uint axis;
    uint level;
};

bool flag_test(uint flags, uint bit) {
    return (flags & bit) != 0u;
}

LineData decode_grid(uint line_id, uint lines) {
    LineData line;
    line.axis = line_id & 1u;
    uint rest = line_id >> 1u;
    line.level = STEPS_DRAWN - 1u - min(rest / lines, STEPS_DRAWN - 1u);
    uint index = rest % lines;
    float half_lines = max(float(lines >> 1u), 1.0);
    line.p = float2((float(index) - half_lines) / half_lines, 0.0);
    return line;
}

LineData decode_axis(uint line_id) {
    LineData line;
    line.axis = line_id;
    line.level = STEPS_DRAWN - 1u;
    line.p = float2(0.0);
    return line;
}

float pow3(float x) {
    return x * x * x;
}

float4 unpack(uint color) {
    return unpackUnorm4x8ToFloat(color);
}

float4 projected(RikkaViewRef view, float3 world, float bias, bool behind) {
    float4 position = mul(view.view, float4(world, 1.0));
    position.xyz *= 1.0 + bias;
    float4 clip = mul(view.projection, position);
    if (behind) {
        clip.z = 0.0;
    }
    return clip;
}

float2 screen(RikkaViewRef view, float4 clip) {
    return (clip.xy / clip.w * 0.5 + 0.5) * view.viewport;
}

[shader("vertex")]
GridVaryings vertex_main(uint vertex_index: SV_VulkanVertexID, out float4 position: SV_Position) {
    RikkaFrameRef frame = push.frame;
    RikkaViewRef view = frame.view;
    RikkaGrid grid = rikka_editor(frame).grid;
    uint flags = push.axes != 0u ? grid.axis_flags : grid.grid_flags;
    bool show_grid = flag_test(flags, SHOW_GRID);
    uint line_id = vertex_index / 6u;
    uint corner = vertex_index % 6u;

    GridVaryings out;
    out.across = 0.0;
    out.along = 0.0;
    out.coord = float2(0.0);
    out.world = float3(0.0);
    out.color = float4(0.0);
    out.on_floor = 0u;
    position = CULLED;

    LineData line = show_grid ? decode_grid(line_id, grid.lines) : decode_axis(line_id);
    bool level_offset = !flag_test(flags, ALIGNED);
    int level = int(grid.level) + int(line.level) - (level_offset ? 1 : 0);
    level = clamp(level, 0, int(STEPS) - 1);
    float step_size = grid.steps[level];
    if (step_size == 0.0) {
        return out;
    }
    float3 eye = view.camera_position;
    uint u = grid.axes[0];
    uint v = grid.axes[1];
    uint up = grid.axes[2];

    float2 step_offs = grid.offset;
    float lines_half = max(float(grid.lines >> 1u), 1.0);
    float3 ends[2];
    float2 coords[2];
    float fixed_coordinate = 0.0;
    uint world_axis = 0u;
    for (uint side = 0u; side < 2u; side++) {
        float2 p;
        if (show_grid) {
            p = line.axis == 0u ? float2(line.p.x, side == 0u ? -1.0 : 1.0)
                                : float2(side == 0u ? -1.0 : 1.0, line.p.x);
            float2 offs = step_offs;
            if (line.axis != 0u) {
                offs.y = round(offs.y / step_size) * step_size;
            } else {
                offs.x = round(offs.x / step_size) * step_size;
            }
            coords[side] = p;
            p = offs + step_size * p * lines_half;
            float2 clip_min = offs - grid.clip;
            float2 clip_max = offs + grid.clip;
            if (all(p < clip_min) || all(p > clip_max)) {
                return out;
            }
            p = clamp(p, clip_min, clip_max);
            float3 world = float3(0.0);
            world[u] = p.x;
            world[v] = p.y;
            ends[side] = world;
            fixed_coordinate = line.axis == 0u ? p.x : p.y;
            world_axis = line.axis == 0u ? v : u;
        } else {
            if (!flag_test(flags, AXIS_X << line.axis)) {
                return out;
            }
            float offs = eye[line.axis];
            float along_axis = offs + step_size * (side == 0u ? -1.0 : 1.0) * lines_half;
            along_axis = clamp(along_axis, offs - grid.clip, offs + grid.clip);
            float3 world = float3(0.0);
            world[line.axis] = along_axis;
            ends[side] = world;
            coords[side] = float2(side == 0u ? -1.0 : 1.0, 0.0);
            world_axis = line.axis;
        }
    }

    bool on_axis = false;
    if (show_grid) {
        on_axis = abs(fixed_coordinate) <= AXIS_EPSILON && flag_test(flags, AXIS_X << world_axis);
        if (on_axis) {
            return out;
        }
        if (line.level < STEPS_DRAWN - 1u && level < int(STEPS) - 1) {
            float j = abs(fixed_coordinate) / grid.steps[level + 1];
            if (abs(floor(j) - j) <= 1e-4) {
                return out;
            }
        }
    }

    float4 color;
    if (!show_grid) {
        color = unpack(grid.colors[2u + world_axis]);
    } else {
        float emphasis = saturate(float(line.level) - frac(grid.level));
        color = lerp(unpack(grid.colors[0]), unpack(grid.colors[1]), emphasis);
    }
    float alpha = saturate(float(line.level) + 1.0 - frac(grid.level));
    bool perspective = flag_test(flags, PERSPECTIVE);
    if (!perspective) {
        alpha *= smoothstep(step_size * 0.25, step_size * pow3(0.25), grid.pixel_fac);
    }
    color.a *= alpha;

    float bias = 0.0;
    if (perspective) {
        float z_factor = float(push.iteration * STEPS_DRAWN + line.level) / float(ITERATIONS * STEPS_DRAWN);
        bias = lerp(Z_MAX_OFFSET, Z_MIN_OFFSET, z_factor);
    }
    bool behind = !perspective && flag_test(flags, BEHIND_GEOMETRY);
    float4 start = projected(view, ends[0], bias, behind);
    float4 end = projected(view, ends[1], bias, behind);
    float3 start_world = ends[0];
    float3 end_world = ends[1];
    float2 start_coord = coords[0];
    float2 end_coord = coords[1];
    float start_ahead = start.w - start.z;
    float end_ahead = end.w - end.z;
    if (start_ahead < 0.0 && end_ahead < 0.0) {
        return out;
    }
    if (start_ahead < 0.0) {
        float t = start_ahead / (start_ahead - end_ahead);
        start = lerp(start, end, t);
        start_world = lerp(start_world, end_world, t);
        start_coord = lerp(start_coord, end_coord, t);
    } else if (end_ahead < 0.0) {
        float t = end_ahead / (end_ahead - start_ahead);
        end = lerp(end, start, t);
        end_world = lerp(end_world, start_world, t);
        end_coord = lerp(end_coord, start_coord, t);
    }
    if (!(start.w > 0.0) || !(end.w > 0.0)) {
        return out;
    }

    float2 from = screen(view, start);
    float2 to = screen(view, end);
    float2 delta = to - from;
    float span = length(delta);
    float2 direction = span > 1e-4 ? delta / span : float2(1.0, 0.0);
    float2 normal = float2(-direction.y, direction.x);
    float width = max(grid.width, 1.0);
    bool aligned = flag_test(flags, ALIGNED);
    float extent = width * 0.5 + (aligned ? 0.0 : FRINGE);
    bool at_end = CORNER_END[corner] == 1u;
    float side = CORNER_SIDE[corner];
    float4 anchor = at_end ? end : start;
    position = float4(anchor.xy + normal * (side * extent) * 2.0 * view.inverse_viewport * anchor.w, anchor.zw);
    out.across = side * extent;
    out.along = at_end ? span : 0.0;
    out.coord = at_end ? end_coord : start_coord;
    out.world = at_end ? end_world : start_world;
    out.color = color;
    out.on_floor = (ends[0][up] == 0.0 && ends[1][up] == 0.0) ? 1u : 0u;
    return out;
}

[shader("fragment")]
float4 fragment_main(GridVaryings varyings) : SV_Target {
    RikkaFrameRef frame = push.frame;
    rikka_clip(frame, varyings.world);
    RikkaViewRef view = frame.view;
    RikkaGrid grid = rikka_editor(frame).grid;
    uint flags = push.axes != 0u ? grid.axis_flags : grid.grid_flags;
    uint up = grid.axes[2];
    float4 color = varyings.color;
    if (flag_test(flags, PERSPECTIVE)) {
        color.a *= 1.0 - min(1.0, length(varyings.coord));
        float3 toward = view.camera_position - varyings.world;
        float dist = length(toward);
        toward /= max(dist, 1e-12);
        if (varyings.on_floor != 0u) {
            color.a *= 1.0 - pow3(1.0 - abs(toward[up]));
        }
        float far_clip = grid.far_clip;
        color.a *= 1.0 - smoothstep(0.0, 0.5 * far_clip, dist - 0.5 * far_clip);
    } else {
        if (!flag_test(flags, ALIGNED)) {
            float length_fade = 1.0 - min(1.0, dot(varyings.coord, varyings.coord));
            color.a *= length_fade * length_fade;
        }
        if (up == 1u && flag_test(flags, SHOW_GRID)) {
            float3 toward = -view.camera_forward;
            color.a *= 1.0 - pow3(1.0 - abs(toward[up]));
        }
    }
    if (color.a < FADE_START && color.a / FADE_START < frac(varyings.along / DASH_WIDTH)) {
        discard;
    }
    if (flag_test(flags, PERSPECTIVE)) {
        color.a *= ADDITIVE_ALPHA[min(push.iteration, ITERATIONS - 1u)];
    }
    if (!flag_test(flags, ALIGNED)) {
        float half_width = max(grid.width, 1.0) * 0.5;
        color.a *= clamp(half_width + 0.5 - abs(varyings.across), 0.0, 1.0);
    }
    if (!(color.a > 0.0)) {
        discard;
    }
    return float4(color.rgb * color.a, color.a);
}
