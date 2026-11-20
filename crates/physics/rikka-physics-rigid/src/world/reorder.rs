use super::RigidWorld;
use crate::body::NONE;
use crate::set::AWAKE_SET;

const REORDER_PERIOD: u64 = 32;
const VISITED: u32 = 1 << 31;

impl RigidWorld {
    fn sort_awake_contacts(&mut self) {
        if self.awake_contacts.is_sorted() {
            return;
        }
        self.awake_contacts.sort_unstable();
        for (index, &contact) in self.awake_contacts.iter().enumerate() {
            self.contacts.get_mut(contact).set_index = index as u32;
        }
    }

    pub(crate) fn reorder_if_due(&mut self) {
        if !self.step_index.is_multiple_of(REORDER_PERIOD) {
            return;
        }
        self.sort_awake_contacts();
        let mut order = std::mem::take(&mut self.solver.split_bodies);
        order.clear();
        order.extend(0..self.sets.awake().bodies.len() as u32);
        let bodies = &self.bodies;
        let ids = &self.sets.awake().bodies;
        let key = |local: u32| {
            let body = ids[local as usize];
            let island = bodies.get(body).island;
            (island == NONE, island, body)
        };
        if order.windows(2).all(|pair| key(pair[0]) <= key(pair[1])) {
            self.solver.split_bodies = order;
            return;
        }
        order.sort_unstable_by_key(|&local| key(local));
        let awake = self.sets.awake_mut();
        for start in 0..order.len() {
            let first = order[start];
            if first & VISITED != 0 || first as usize == start {
                order[start] = first | VISITED;
                continue;
            }
            let saved_pose = awake.poses[start];
            let saved_motion = awake.motion[start];
            let saved_state = awake.states[start];
            let saved_body = awake.bodies[start];
            let mut current = start;
            loop {
                let next = (order[current] & !VISITED) as usize;
                order[current] |= VISITED;
                if next == start {
                    awake.poses[current] = saved_pose;
                    awake.motion[current] = saved_motion;
                    awake.states[current] = saved_state;
                    awake.bodies[current] = saved_body;
                    break;
                }
                awake.poses[current] = awake.poses[next];
                awake.motion[current] = awake.motion[next];
                awake.states[current] = awake.states[next];
                awake.bodies[current] = awake.bodies[next];
                current = next;
            }
        }
        for local in 0..awake.bodies.len() {
            let body = self.sets.awake().bodies[local];
            self.bodies.get_mut(body).local = local as u32;
        }
        for local in 0..self.sets.awake().bodies.len() {
            let body = self.sets.awake().bodies[local];
            self.refresh_solver_indices(body);
        }
        self.graph.version += 1;
        self.solver.split_bodies = order;
        for color in 0..self.graph.colors.len() {
            let contacts = &mut self.graph.colors[color].contacts;
            let records = &self.contacts;
            contacts.sort_unstable_by_key(|&id| {
                let record = records.get(id);
                (record.solver_a.max(record.solver_b), id)
            });
            for (index, &id) in self.graph.colors[color].contacts.iter().enumerate() {
                self.contacts.get_mut(id).color_index = index as u32;
            }
            let joints = &mut self.graph.colors[color].joints;
            let records = &self.joints;
            joints.sort_unstable_by_key(|&id| {
                let record = records.get(id);
                (record.solver_a.max(record.solver_b), id)
            });
            for (index, &id) in self.graph.colors[color].joints.iter().enumerate() {
                self.joints.get_mut(id).local = index as u32;
            }
        }
        debug_assert!(
            self.sets
                .awake()
                .bodies
                .iter()
                .all(|&body| self.bodies.get(body).set == AWAKE_SET)
        );
    }
}
