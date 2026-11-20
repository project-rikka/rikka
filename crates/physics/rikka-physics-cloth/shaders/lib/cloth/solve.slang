import cloth.solve.positions;
import cloth.solve.predict;
import cloth.solve.constraints;
import cloth.solve.settle;
import cloth.solve.retidy;
import cloth.solve.finish;

void solve(uint group, uint local) {
    job = pc.jobs[pc.first_job + group];
    lane = local;
    first = job.range.x;
    count = job.range.y;
    if (lane == 0u) {
        shared_store(__getAddress(fastest), 0u);
        shared_store(__getAddress(jittering), 0u);
        shared_store(__getAddress(moving), 0u);
    }
    planned = min(2u * (job.colours.y + job.colours.z), PLANNED);
    for (uint at = lane; at < planned; at += THREADS) {
        shared_store(__getAddress(ranges[at]), job.plan[job.colours.x + at]);
    }
    Positions.load();
    sync();
    float h = job.step.x;
    uint substeps = job.steps.y;
    bool measuring = job.steps.w != NONE;
    for (uint s = 0u; s < substeps; ++s) {
        uint substep = job.steps.x + s;
        float before = float(s) / float(substeps);
        float alpha = float(s + 1u) / float(substeps);
        float time = job.step.y + float(substep + 1u) * h;
        predict(substep, alpha, h, time);
        sync();
        stretch(h);
        jacobi(alpha, h);
        sync();
        settle(alpha, before, h);
        sync();
        if (job.flags.w != 0u) {
            grip(alpha, false);
            sync();
        }
        finish(h);
        sync();
        bool last = s + 1u == substeps;
        retidy(last);
        post_project(alpha, last);
        sync();
        if (last) {
            retidy_edges(false, true);
        }
        if (job.flags.w != 0u) {
            grip(alpha, true);
            sync();
        }
    }
    float span = float(substeps) * h;
    for (uint i = first + lane; i < first + count; i += THREADS) {
        float3 x = position(i);
        bool movable = job.traits[i].a.x != 0.0;
        float wavering = movable ? steadied(i, x) : 0.0;
        Positions.store(i, x);
        if (measuring && movable) {
            float speed = distance(x, job.epoch_start[i].xyz) / span;
            shared_atomic_max(__getAddress(fastest), asuint(speed));
            shared_atomic_max(__getAddress(jittering), asuint(speed * wavering));
            if (speed >= STIRRED_SPEED) {
                shared_atomic_add(__getAddress(moving), 1u);
            }
        }
    }
    sync();
    if (measuring && lane == 0u) {
        queue_atomic_max(job.stats.at(STATS_WORDS * job.steps.w), shared_load(__getAddress(fastest)));
        queue_atomic_max(job.stats.at(STATS_WORDS * job.steps.w + 1u), shared_load(__getAddress(jittering)));
        queue_atomic_add(job.stats.at(STATS_WORDS * job.steps.w + 2u), shared_load(__getAddress(moving)));
    }
}
