use rikka_physics_core::Stopwatch;

use super::World;
use crate::call::Call;
use crate::history::Scope;

impl World {
    pub fn advance(&mut self, frame_dt: f64) {
        self.log(Scope::Input, || Call::Advance(frame_dt));
        let ticks = self.clock.plan_frame(frame_dt, self.settings.max_ticks);
        self.run_ticks(ticks);
    }

    pub fn advance_to(&mut self, motion_time: f64) {
        self.log(Scope::Input, || Call::AdvanceTo(motion_time));
        let ticks = self.clock.plan_until(motion_time);
        self.run_ticks(ticks);
    }

    fn run_ticks(&mut self, ticks: u32) {
        self.events.clear();
        self.tick_hashes.clear();
        for _ in 0..ticks {
            self.tick();
        }
        if ticks > 0 {
            self.targets.finish_frame();
        }
        self.clock.finish_frame();
    }

    fn tick(&mut self) {
        self.history.start();
        let h = self.settings.tick_seconds();
        let fraction = self.clock.fraction();
        self.profile.time("bone targets", || {
            self.targets.apply(&mut self.rigid, fraction);
        });
        self.profile.time("rigid step", || self.rigid.step(h));
        self.clock.finish_tick();
        self.write_back();
    }

    fn write_back(&mut self) {
        let tick = self.clock.tick();
        let mut watch = Stopwatch::start(&self.profile);
        self.poses.record(self.rigid.body_move_events(), tick);
        self.events.collect(&self.rigid);
        watch.lap(&mut self.profile, "write back");
        if self.recorder.is_some() || self.replaying {
            let hash = self.state_hash();
            self.tick_hashes.push(hash);
            if let Some(recorder) = &mut self.recorder {
                recorder.push_tick(hash);
            }
        }
    }
}
