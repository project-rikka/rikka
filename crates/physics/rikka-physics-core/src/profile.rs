use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileEntry {
    pub name: &'static str,
    pub duration: Duration,
    pub count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Profile {
    enabled: bool,
    entries: Vec<ProfileEntry>,
}

impl Profile {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            entries: Vec::new(),
        }
    }

    pub const fn enabled() -> Self {
        Self {
            enabled: true,
            entries: Vec::new(),
        }
    }

    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    #[inline]
    pub fn record(&mut self, name: &'static str, duration: Duration) {
        if !self.enabled {
            return;
        }
        match self.entries.iter_mut().find(|entry| entry.name == name) {
            Some(entry) => {
                entry.duration += duration;
                entry.count += 1;
            }
            None => self.entries.push(ProfileEntry {
                name,
                duration,
                count: 1,
            }),
        }
    }

    #[inline]
    pub fn scope(&mut self, name: &'static str) -> ProfileScope<'_> {
        let start = self.enabled.then(Instant::now);
        ProfileScope {
            profile: self,
            name,
            start,
        }
    }

    #[inline]
    pub fn time<R>(&mut self, name: &'static str, work: impl FnOnce() -> R) -> R {
        let _scope = self.scope(name);
        work()
    }

    pub fn reset(&mut self) {
        for entry in &mut self.entries {
            entry.duration = Duration::ZERO;
            entry.count = 0;
        }
    }

    pub fn accumulate(&mut self, other: &Self) {
        for entry in &other.entries {
            match self.entries.iter_mut().find(|own| own.name == entry.name) {
                Some(own) => {
                    own.duration += entry.duration;
                    own.count += entry.count;
                }
                None => self.entries.push(*entry),
            }
        }
    }

    pub fn entries(&self) -> &[ProfileEntry] {
        &self.entries
    }

    pub fn get(&self, name: &str) -> Duration {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map_or(Duration::ZERO, |entry| entry.duration)
    }
}

#[derive(Debug)]
pub struct ProfileScope<'a> {
    profile: &'a mut Profile,
    name: &'static str,
    start: Option<Instant>,
}

impl Drop for ProfileScope<'_> {
    #[inline]
    fn drop(&mut self) {
        if let Some(start) = self.start {
            self.profile.record(self.name, start.elapsed());
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Stopwatch(Option<Instant>);

impl Stopwatch {
    #[inline]
    pub fn start(profile: &Profile) -> Self {
        Self(profile.is_enabled().then(Instant::now))
    }

    #[inline]
    pub fn lap(&mut self, profile: &mut Profile, name: &'static str) {
        if let Some(start) = self.0 {
            let now = Instant::now();
            profile.record(name, now - start);
            self.0 = Some(now);
        }
    }
}
