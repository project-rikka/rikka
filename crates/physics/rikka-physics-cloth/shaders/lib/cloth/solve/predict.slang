import cloth.solve.positions;

float3 triangle_force(float3 air, float3 doubled_normal, float drag, float lift) {
    float doubled_area = length(doubled_normal);
    float speed_squared = dot(air, air);
    if (!(doubled_area > 1.0e-20) || !(speed_squared > 1.0e-12)) {
        return float3(0.0);
    }
    float3 normal = doubled_normal / doubled_area;
    float speed = sqrt(speed_squared);
    float3 flow = air / speed;
    float facing = dot(normal, flow);
    float cosine = abs(facing);
    float pressure = 0.25 * AIR_DENSITY * doubled_area * speed_squared * cosine;
    float3 downstream = normal * (facing >= 0.0 ? 1.0 : -1.0);
    float3 across = downstream - flow * cosine;
    float across_squared = dot(across, across);
    float3 lift_direction = across_squared > 1.0e-12 ? across * rsqrt(across_squared) : float3(0.0);
    float sine = sqrt(max(1.0 - cosine * cosine, 0.0));
    return flow * (pressure * drag) + lift_direction * (pressure * lift * sine);
}

void predict(uint substep, float alpha, float h, float time) {
    float total = float(job.steps.z);
    float3 wind = job.wind.xyz;
    float gusting = job.wind.w * length(wind);
    for (uint i = first + lane; i < first + count; i += THREADS) {
        Trait trait = job.traits[i];
        float3 x = position(i);
        if (trait.a.x == 0.0) {
            job.starts[i] = float4(x, 0.0);
            float3 pinned = target(i, alpha);
            job.preds[i] = float4(pinned, 0.0);
            place(i, pinned);
            continue;
        }
        Piece piece = job.pieces[trait.d.x];
        float carry = trait.c.x;
        float moved = lerp(piece.scaled.w, 1.0, carry);
        float turned = lerp(piece.centre.w, 1.0, carry);
        float3 motion = piece.scaled.xyz + piece.fixed_shift.xyz;
        float3 centre = piece.centre.xyz + motion * (float(substep) / total);
        float3 shift = (piece.scaled.xyz * moved + piece.fixed_shift.xyz) / total;
        float angle = piece.turn.w * turned / total;
        float4 stored = job.velocities[i];
        float3 v = stored.xyz;
        x = centre + rotate_about(x - centre, piece.turn.xyz, angle) + shift;
        v = rotate_about(v, piece.turn.xyz, angle);
        job.starts[i] = float4(x, 0.0);
        v *= damped(trait.a.y, h);
        if (piece.settling.x > 0.0) {
            float substep_time = total * h;
            float3 frame_velocity = piece.scaled.xyz * ((1.0 - moved) / substep_time)
                + cross(piece.turn.xyz, x - centre) * (piece.turn.w * (1.0 - turned) / substep_time);
            v = frame_velocity + (v - frame_velocity) * damped(piece.settling.x, h);
        }
        float3 acceleration = job.gravity.xyz * piece.fixed_shift.w;
        float3 normal = job.normals[i].xyz;
        if (trait.b.x > 0.0 && (piece.response.z > 0.0 || piece.response.w > 0.0)) {
            float3 air = gusting > 0.0 ? wind + gusting * vector_noise(x * WIND_SCALE + float3(time * WIND_DRIFT)) : wind;
            float3 force = triangle_force(air - v, normal * trait.b.x, piece.response.z, piece.response.w);
            acceleration += force * trait.c.w;
        }
        float3 spin = piece.spin.xyz;
        float spin_squared = dot(spin, spin);
        if (spin_squared > 0.0) {
            float3 axis = spin * rsqrt(spin_squared);
            float3 out_of_axis = (x - centre) - axis * dot(x - centre, axis);
            acceleration += piece.spin.w * spin_squared * out_of_axis;
        }
        float breeze = min(length(wind) / FLUTTER_WIND, 1.0);
        if (piece.flutter.x != 0.0 && piece.flutter.z > 0.0 && breeze > 0.0) {
            float phase = 2.0 * PI * (piece.flutter.y * time - trait.c.y / piece.flutter.z);
            acceleration += piece.flutter.x * breeze * asfloat(trait.d.w) * sin(phase) * normal;
        }
        float3 coasting = x + v * h;
        v += acceleration * h;
        x += v * h;
        job.preds[i] = float4(coasting, stored.w);
        place(i, x);
    }
}
