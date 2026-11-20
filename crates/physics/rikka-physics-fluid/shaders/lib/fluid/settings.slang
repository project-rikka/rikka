__exported import physics.memory;

static const uint MAX_BONDS = 12u;
static const uint MAX_SUBSTANCES = 32u;
static const uint DEAD = 0x80000000u;
static const uint FRESH = 0x40000000u;
static const uint PINNED = 0x20000000u;
static const uint SUBSTANCE_MASK = 0xffu;
static const uint NONE = 0xffffffffu;
static const float FOREVER = 1.0e30;

struct Substance {
    float4 flow;
    float4 stick;
    float4 bond;
    float4 tint;
    float4 contact;
};

struct Substances {
    Substance* address;

    __subscript(uint k)->Substance {
        get {
            Substance* substance = address + k;
            return {
                load_non_writable<16>(__getAddress(substance->flow)),
                load_non_writable<16>(__getAddress(substance->stick)),
                load_non_writable<16>(__getAddress(substance->bond)),
                load_non_writable<16>(__getAddress(substance->tint)),
                load_non_writable<16>(__getAddress(substance->contact)),
            };
        }
    }
}

struct SettingsBlock {
    float4 kernel;
    float4 solver;
    float4 body;
    float4 bounds_lo;
    float4 bounds_hi;
    uint4 grid;
    float4 grid_cell;
    uint4 tri_grid;
    float4 tri_cell;
    float4 look;
    float4 look_limits;
    float4 foam_potential;
    float4 foam_energy;
    float4 foam_motion;
    float4 foam_kinds;
    float4 boundary;
    float4 wall_shape;
    float4 closing;
    float4 reaction;
    float4 wall[32];
    Substance substances[MAX_SUBSTANCES];
    float4 surface[8];
};

struct Settings {
    SettingsBlock* block;

    property float4 kernel {
        get {
            return load_non_writable<16>(__getAddress(block->kernel));
        }
    }

    property float4 solver {
        get {
            return load_non_writable<16>(__getAddress(block->solver));
        }
    }

    property float4 body {
        get {
            return load_non_writable<16>(__getAddress(block->body));
        }
    }

    property float4 bounds_lo {
        get {
            return load_non_writable<16>(__getAddress(block->bounds_lo));
        }
    }

    property float4 bounds_hi {
        get {
            return load_non_writable<16>(__getAddress(block->bounds_hi));
        }
    }

    property uint4 grid {
        get {
            return load_non_writable<16>(__getAddress(block->grid));
        }
    }

    property float4 grid_cell {
        get {
            return load_non_writable<16>(__getAddress(block->grid_cell));
        }
    }

    property uint4 tri_grid {
        get {
            return load_non_writable<16>(__getAddress(block->tri_grid));
        }
    }

    property float4 tri_cell {
        get {
            return load_non_writable<16>(__getAddress(block->tri_cell));
        }
    }

    property float4 look {
        get {
            return load_non_writable<16>(__getAddress(block->look));
        }
    }

    property float4 look_limits {
        get {
            return load_non_writable<16>(__getAddress(block->look_limits));
        }
    }

    property float4 foam_potential {
        get {
            return load_non_writable<16>(__getAddress(block->foam_potential));
        }
    }

    property float4 foam_energy {
        get {
            return load_non_writable<16>(__getAddress(block->foam_energy));
        }
    }

    property float4 foam_motion {
        get {
            return load_non_writable<16>(__getAddress(block->foam_motion));
        }
    }

    property float4 foam_kinds {
        get {
            return load_non_writable<16>(__getAddress(block->foam_kinds));
        }
    }

    property float4 boundary {
        get {
            return load_non_writable<16>(__getAddress(block->boundary));
        }
    }

    property float4 wall_shape {
        get {
            return load_non_writable<16>(__getAddress(block->wall_shape));
        }
    }

    property float4 closing {
        get {
            return load_non_writable<16>(__getAddress(block->closing));
        }
    }

    property float4 reaction {
        get {
            return load_non_writable<16>(__getAddress(block->reaction));
        }
    }

    property NonWritableBuffer<float4, 16> wall {
        get {
            return { __getAddress(block->wall[0]) };
        }
    }

    property Substances substances {
        get {
            return { __getAddress(block->substances[0]) };
        }
    }

    property NonWritableBuffer<float4, 16> surface {
        get {
            return { __getAddress(block->surface[0]) };
        }
    }
}

float kernel_h(Settings s) {
    return s.kernel.x;
}

float kernel_inv_h(Settings s) {
    return s.kernel.y;
}

float kernel_h2(Settings s) {
    return s.kernel.z;
}

float kernel_inv_s0(Settings s) {
    return s.kernel.w;
}

float2 wall_density(Settings s, float gap) {
    float depth = max(gap - s.body.x + s.wall_shape.x, 0.0);
    float at = clamp(depth / s.wall_shape.y, 0.0, 62.999);
    uint low = uint(at);
    float t = at - float(low);
    float4 first = s.wall[low / 2u];
    float4 second = s.wall[(low + 1u) / 2u];
    float2 a = (low % 2u) == 0u ? first.xy : first.zw;
    float2 b = ((low + 1u) % 2u) == 0u ? second.xy : second.zw;
    return lerp(a, b, t);
}

float surface_completeness(Settings s, float openness) {
    float at = clamp(openness * 31.0, 0.0, 30.999);
    uint low = uint(at);
    float t = at - float(low);
    float a = s.surface[low / 4u][low % 4u];
    float b = s.surface[(low + 1u) / 4u][(low + 1u) % 4u];
    return lerp(a, b, t);
}

float closing_constraint(Settings s, float relative, float steepness, float dilation) {
    float openness = steepness * s.closing.x;
    float interior = clamp(1.0 - openness * s.closing.z, 0.0, 1.0);
    float stretched = clamp((dilation - 1.0) / s.closing.w, 0.0, 1.0);
    float deficit = min(relative / surface_completeness(s, openness) - 1.0, 0.0);
    return max(deficit, -s.closing.y * interior * stretched);
}

bool is_dead(uint phase) {
    return (phase & DEAD) != 0u;
}

uint substance_of(uint phase) {
    return phase & SUBSTANCE_MASK;
}

float kernel_w(float r, float inv_h) {
    float q = max(1.0 - r * inv_h, 0.0);
    return q * q;
}

float kernel_dw(float r, float inv_h) {
    return -2.0 * max(1.0 - r * inv_h, 0.0) * inv_h;
}

float cohesion_shape(float r, float inv_h) {
    float q = r * inv_h;
    if (q >= 1.0) {
        return 0.0;
    }
    float a = (1.0 - q) * q;
    float cubed = a * a * a;
    if (q > 0.5) {
        return 64.0 * cubed;
    }
    return 128.0 * cubed - 1.0;
}

float3 landed(float3 x0, float3 x, float ground, float mu) {
    float depth = ground - x.y;
    x.y = ground;
    float2 slide = x.xz - x0.xz;
    float length_slide = length(slide);
    x.xz = length_slide <= mu * depth ? x0.xz : x.xz - slide * (mu * depth / length_slide);
    return x;
}

float3 dragged(float3 x0, float3 x, float3 normal, float3 travel, float rate) {
    float3 moved = (x - x0) - travel;
    float3 tangential = moved - normal * dot(moved, normal);
    return x - tangential * (1.0 - exp(-rate));
}

float3 floored(Settings s, float3 x0, float3 x, uint phase) {
    float ground = s.body.z + s.bounds_lo.w;
    if (s.body.w <= 0.5 || x.y >= ground) {
        return x;
    }
    return landed(x0, x, ground, s.substances[substance_of(phase)].contact.x);
}

bool mixed(Settings s) {
    return s.wall_shape.z > 1.5;
}

uint kind_of(Settings s, ReadUints phases, uint j, uint fallback) {
    return mixed(s) ? substance_of(phases[j]) : fallback;
}

bool outside_bounds(Settings s, float3 x) {
    return any(x < s.bounds_lo.xyz) || any(x > s.bounds_hi.xyz);
}
