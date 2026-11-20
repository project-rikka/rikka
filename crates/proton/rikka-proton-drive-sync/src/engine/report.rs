use std::time::Duration;

use jiff::Timestamp;
use rikka_proton_drive::{DeviceId, DeviceSync, DriveClient};
use tokio::time::Instant;

const REPORT_INTERVAL: Duration = Duration::from_secs(300);

pub(crate) struct DeviceReport {
    client: DriveClient,
    device: DeviceId,
    last_sent: Option<Instant>,
    pending_sync_time: Option<Timestamp>,
    dirty: bool,
}

impl DeviceReport {
    pub(crate) fn new(client: DriveClient, device: DeviceId) -> DeviceReport {
        DeviceReport {
            client,
            device,
            last_sent: None,
            pending_sync_time: None,
            dirty: true,
        }
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
        self.pending_sync_time = None;
    }

    pub(crate) fn mark_idle(&mut self, clean: bool) {
        if clean && self.dirty {
            self.dirty = false;
            self.pending_sync_time = Some(Timestamp::now());
        }
    }

    pub(crate) fn next_report(&self) -> Option<Instant> {
        self.pending_sync_time?;
        Some(
            self.last_sent
                .map_or_else(Instant::now, |last_sent| last_sent + REPORT_INTERVAL),
        )
    }

    pub(crate) async fn send_due(&mut self) {
        if self
            .last_sent
            .is_none_or(|last_sent| last_sent.elapsed() >= REPORT_INTERVAL)
        {
            self.flush().await;
        }
    }

    pub(crate) async fn flush(&mut self) {
        let Some(synced_at) = self.pending_sync_time else {
            return;
        };
        self.last_sent = Some(Instant::now());
        match self
            .client
            .set_device_sync(&self.device, DeviceSync::On, Some(synced_at))
            .await
        {
            Ok(()) => self.pending_sync_time = None,
            Err(error) => {
                tracing::warn!(%error, "failed to report device sync time")
            }
        }
    }
}
