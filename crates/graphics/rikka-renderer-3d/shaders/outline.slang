import rikka.outline;

struct Push {
    uint identity;
    uint reach;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

uint identity_at(int2 texel, int2 last) {
    return rikka_identity_images[push.identity].Load(int3(clamp(texel, int2(0), last), 0)).r;
}

bool boundary(uint a, uint b) {
    return a != b && ((a | b) & RIKKA_IDENTITY_SELECTED) != 0u;
}

[shader("fragment")]
uint fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    int2 centre = int2(frag_coord.xy);
    int2 last = int2(push.size) - 1;
    int reach = int(push.reach);

    uint left = identity_at(centre + int2(-reach - 1, 0), last);
    uint here = identity_at(centre + int2(-reach, 0), last);
    uint nearest = RIKKA_OUTLINE_NONE;
    for (int x = -reach; x <= reach; ++x) {
        int2 at = centre + int2(x, 0);
        uint right = identity_at(at + int2(1, 0), last);
        bool edge = boundary(here, left) || boundary(here, right)
            || boundary(here, identity_at(at + int2(0, 1), last))
            || boundary(here, identity_at(at - int2(0, 1), last));
        if (edge) {
            nearest = min(nearest, uint(abs(x)));
        }
        left = here;
        here = right;
    }
    return nearest;
}
