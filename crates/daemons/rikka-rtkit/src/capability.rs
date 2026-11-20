use std::io;

use rustix::thread::{CapabilitySet, CapabilitySets, capabilities, set_capabilities};

pub fn drop_sys_nice() -> io::Result<bool> {
    let sets = capabilities(None)?;
    if !sets.permitted.contains(CapabilitySet::SYS_NICE) {
        return Ok(false);
    }
    set_capabilities(
        None,
        CapabilitySets {
            effective: sets.effective.difference(CapabilitySet::SYS_NICE),
            permitted: sets.permitted.difference(CapabilitySet::SYS_NICE),
            inheritable: sets.inheritable.difference(CapabilitySet::SYS_NICE),
        },
    )?;
    Ok(true)
}
