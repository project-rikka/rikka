use super::body_data::{BodyMotion, BodyPose};
use crate::solver::state::BodyState;

pub(crate) const STATIC_SET: u32 = 0;
pub(crate) const DISABLED_SET: u32 = 1;
pub(crate) const AWAKE_SET: u32 = 2;
pub(crate) const FIRST_SLEEPING_SET: u32 = 3;

#[derive(Debug, Default)]
pub(crate) struct SolverSet {
    pub bodies: Vec<u32>,
    pub poses: Vec<BodyPose>,
    pub motion: Vec<BodyMotion>,
    pub states: Vec<BodyState>,
    pub contacts: Vec<u32>,
    pub joints: Vec<u32>,
    pub islands: Vec<u32>,
}

impl SolverSet {
    pub(crate) fn push_body(
        &mut self,
        body: u32,
        pose: BodyPose,
        motion: BodyMotion,
        state: BodyState,
    ) -> u32 {
        let local = self.bodies.len() as u32;
        self.bodies.push(body);
        self.poses.push(pose);
        self.motion.push(motion);
        self.states.push(state);
        local
    }

    pub(crate) fn remove_body(&mut self, local: u32) -> Option<u32> {
        let index = local as usize;
        self.bodies.swap_remove(index);
        self.poses.swap_remove(index);
        self.motion.swap_remove(index);
        self.states.swap_remove(index);
        self.bodies.get(index).copied()
    }

    pub(crate) fn clear(&mut self) {
        self.bodies.clear();
        self.poses.clear();
        self.motion.clear();
        self.states.clear();
        self.contacts.clear();
        self.joints.clear();
        self.islands.clear();
    }
}

pub(crate) fn swap_remove(list: &mut Vec<u32>, index: u32) -> Option<u32> {
    list.swap_remove(index as usize);
    list.get(index as usize).copied()
}

#[derive(Debug)]
pub(crate) struct SolverSets {
    pub sets: Vec<SolverSet>,
    free: Vec<u32>,
}

impl Default for SolverSets {
    fn default() -> Self {
        Self {
            sets: (0..FIRST_SLEEPING_SET)
                .map(|_| SolverSet::default())
                .collect(),
            free: Vec::new(),
        }
    }
}

impl SolverSets {
    #[inline]
    pub(crate) fn awake(&self) -> &SolverSet {
        &self.sets[AWAKE_SET as usize]
    }

    #[inline]
    pub(crate) fn awake_mut(&mut self) -> &mut SolverSet {
        &mut self.sets[AWAKE_SET as usize]
    }

    #[inline]
    pub(crate) fn get(&self, set: u32) -> &SolverSet {
        &self.sets[set as usize]
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, set: u32) -> &mut SolverSet {
        &mut self.sets[set as usize]
    }

    pub(crate) fn allocate_sleeping(&mut self) -> u32 {
        match self.free.pop() {
            Some(set) => set,
            None => {
                self.sets.push(SolverSet::default());
                self.sets.len() as u32 - 1
            }
        }
    }

    pub(crate) fn release(&mut self, set: u32) {
        debug_assert!(set >= FIRST_SLEEPING_SET);
        self.sets[set as usize].clear();
        self.free.push(set);
    }
}
