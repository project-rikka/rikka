use super::World;
use crate::call::Call;
use crate::clock::Clock;
use crate::history::{Entry, Scope, Setup};
use crate::{Error, Result};

impl World {
    pub fn reset(&mut self) -> Result<()> {
        self.log(Scope::Input, || Call::Rebuild);
        self.history.start();
        self.rebuild()
    }

    pub fn seek(
        &mut self,
        time: f64,
        preroll: u32,
        mut animate: impl FnMut(&mut World, f64) -> Result<()>,
    ) -> Result<()> {
        if !time.is_finite() {
            return Err(Error::NotFinite { what: "seek time" });
        }
        self.reset()?;
        let rate = self.settings.tick_rate as f64;
        let start = (time * rate).round() as i64 - preroll as i64;
        self.restart(start as f64 / rate);
        animate(self, start as f64 / rate)?;
        self.snap();
        for tick in 1..=preroll as i64 {
            let at = (start + tick) as f64 / rate;
            animate(self, at)?;
            self.advance_to(at);
        }
        Ok(())
    }

    pub(super) fn restart(&mut self, time: f64) {
        self.log(Scope::Input, || Call::Restart(time));
        self.clock.restart(time);
    }

    pub(super) fn snap(&mut self) {
        self.log(Scope::Input, || Call::Snap);
        self.targets.snap(&mut self.rigid);
        self.poses.clear();
    }

    pub(super) fn rebuild(&mut self) -> Result<()> {
        let entries = self.history.take();
        self.rigid = self.settings.rigid_world(self.executor.clone())?;
        self.rigid
            .profile_mut()
            .set_enabled(self.profile.is_enabled());
        self.gravity = self.settings.gravity;
        self.targets.clear();
        self.poses.clear();
        self.events.clear();
        self.bodies.clear();
        self.joints.clear();
        self.characters.reset();
        self.clock = Clock::new(self.settings.tick_rate);
        self.rebuilding = true;
        let result = entries.iter().try_for_each(|entry| match entry {
            Entry::Setup(setup) => self.apply_setup(setup),
            Entry::Call(call) => self.apply_call(call),
        });
        self.rebuilding = false;
        self.history.restore(entries);
        result
    }

    pub(crate) fn apply_setup(&mut self, setup: &Setup) -> Result<()> {
        match setup {
            Setup::Body(body) => self.create_body(body.clone()).map(drop),
            Setup::Shape(body, shape) => self.create_shape(*body, shape.clone()).map(drop),
            Setup::Joint(joint) => self.create_joint(**joint).map(drop),
            Setup::Character(def) => self.create_character(*def).map(drop),
        }
    }
}
