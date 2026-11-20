use rustix::thread::{
    CapabilitySet, CapabilitySets, capabilities, clear_ambient_capability_set, set_capabilities,
};

#[must_use = "CAP_SYS_NICE is dropped from the state thread after output setup"]
#[derive(Debug)]
pub(crate) struct NiceCapability {
    has_sys_nice: bool,
}

impl NiceCapability {
    pub(crate) fn without_inheritance() -> Self {
        if let Err(error) = clear_ambient_capability_set() {
            tracing::warn!("cannot clear ambient capabilities: {error}");
        }
        let sets = match capabilities(None) {
            Ok(sets) => sets,
            Err(error) => {
                tracing::debug!("cannot read process capabilities: {error}");
                return Self {
                    has_sys_nice: false,
                };
            }
        };
        let has_sys_nice = sets.permitted.contains(CapabilitySet::SYS_NICE);
        let reduced = CapabilitySets {
            effective: match has_sys_nice {
                true => sets.effective | CapabilitySet::SYS_NICE,
                false => sets.effective,
            },
            inheritable: CapabilitySet::empty(),
            ..sets
        };
        if reduced != sets
            && let Err(error) = set_capabilities(None, reduced)
        {
            tracing::warn!("cannot drop inheritable capabilities: {error}");
        }
        Self { has_sys_nice }
    }

    pub(crate) fn foretell(&self) {
        match self.has_sys_nice {
            true => tracing::info!(
                "CAP_SYS_NICE held; high-priority GPU queues and frame loop enabled, children do not inherit it"
            ),
            false => tracing::info!(
                "CAP_SYS_NICE not held; GPU queues run at normal priority (grant with `sudo setcap cap_sys_nice+ep {}`)",
                std::env::current_exe().map_or_else(
                    |_| "the compositor's binary".to_owned(),
                    |path| path.display().to_string()
                )
            ),
        }
    }

    pub(crate) fn leave_state_thread(self) {
        if !self.has_sys_nice {
            return;
        }
        match rikka_rtkit::drop_sys_nice() {
            Ok(_) => tracing::debug!("dropped CAP_SYS_NICE on state thread"),
            Err(error) => {
                tracing::warn!("cannot drop CAP_SYS_NICE on state thread: {error}")
            }
        }
    }
}
