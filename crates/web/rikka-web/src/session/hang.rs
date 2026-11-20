use std::sync::Arc;

use rikka_gecko_protocol::Target;
use rikka_gecko_protocol::geckoview::content::{HangReport, HangReportStop, HangReportWait};

use crate::wire::Link;

#[derive(Debug)]
pub struct Hang {
    pub script: String,
    id: u32,
    target: Target,
    link: Arc<Link>,
}

impl Hang {
    pub(crate) fn new(report: HangReport, target: Target, link: &Arc<Link>) -> Hang {
        Hang {
            script: report.script_file_name,
            id: report.hang_id,
            target,
            link: Arc::clone(link),
        }
    }

    pub fn stop_script(self) {
        self.link
            .send(self.target, &HangReportStop { hang_id: self.id });
    }

    pub fn wait(self) {
        self.link
            .send(self.target, &HangReportWait { hang_id: self.id });
    }
}
