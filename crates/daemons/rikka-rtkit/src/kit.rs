use zbus::blocking::{Connection, Proxy};

const SERVICE: &str = "org.freedesktop.RealtimeKit1";
const PATH: &str = "/org/freedesktop/RealtimeKit1";

pub(crate) struct Kit {
    proxy: Proxy<'static>,
}

impl Kit {
    pub(crate) fn connect() -> zbus::Result<Self> {
        let connection = Connection::system()?;
        let proxy = zbus::blocking::proxy::Builder::new(&connection)
            .destination(SERVICE)?
            .path(PATH)?
            .interface(SERVICE)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()?;
        Ok(Self { proxy })
    }

    pub(crate) fn max_realtime_priority(&self) -> zbus::Result<i32> {
        self.proxy.get_property("MaxRealtimePriority")
    }

    pub(crate) fn rttime_usec_max(&self) -> zbus::Result<i64> {
        self.proxy.get_property("RTTimeUSecMax")
    }

    pub(crate) fn min_nice_level(&self) -> zbus::Result<i32> {
        self.proxy.get_property("MinNiceLevel")
    }

    pub(crate) fn make_thread_realtime(&self, priority: u32) -> zbus::Result<()> {
        let (process, thread) = caller();
        self.proxy
            .call_method("MakeThreadRealtimeWithPID", &(process, thread, priority))?;
        Ok(())
    }

    pub(crate) fn make_thread_high_priority(&self, nice: i32) -> zbus::Result<()> {
        let (process, thread) = caller();
        self.proxy
            .call_method("MakeThreadHighPriorityWithPID", &(process, thread, nice))?;
        Ok(())
    }
}

fn caller() -> (u64, u64) {
    let thread = unsafe { libc::gettid() }.cast_unsigned();
    (u64::from(std::process::id()), u64::from(thread))
}
