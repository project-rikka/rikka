pub(crate) const TIMEDATE: &str = "org.freedesktop.timedate1";
pub(crate) const TIMEDATE_PATH: &str = "/org/freedesktop/timedate1";

pub(crate) const TIMESYNC: &str = "org.freedesktop.timesync1";
pub(crate) const TIMESYNC_PATH: &str = "/org/freedesktop/timesync1";
pub(crate) const TIMESYNC_MANAGER: &str = "org.freedesktop.timesync1.Manager";

#[zbus::proxy(
    interface = "org.freedesktop.timedate1",
    default_service = "org.freedesktop.timedate1",
    default_path = "/org/freedesktop/timedate1",
    gen_async = false
)]
pub(crate) trait Timedate {
    #[zbus(name = "SetTime")]
    fn set_time(&self, usec_utc: i64, relative: bool, interactive: bool) -> zbus::Result<()>;
}
