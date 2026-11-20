use music_core::Result;
use music_stream::{Fetcher, Plan, Spool, answer_bytes};

use crate::sealed;

const AGENT: &str = "Mozilla/5.0";
const AHEAD: usize = 3;
const RESUMES: u32 = 2;
const SEGMENT: &str = "$SEGMENT$";

pub(crate) struct Cdn {
    fetcher: Fetcher,
}

impl Cdn {
    pub(crate) fn new() -> Cdn {
        Cdn {
            fetcher: Fetcher::new(AGENT).ahead(AHEAD).tries(1 + RESUMES),
        }
    }

    pub(crate) async fn init(&self, template: &str) -> Result<Vec<u8>> {
        let request = self.fetcher.client().get(address(template, 0));
        Ok(answer_bytes(request).await?.to_vec())
    }

    pub(crate) async fn segments(
        &self,
        head: Vec<u8>,
        template: &str,
        count: usize,
        spool: &mut Spool,
    ) -> Result<()> {
        let plan = (1..=count)
            .fold(Plan::new().lead(head), |plan, index| {
                plan.piece(address(template, index), None)
            })
            .framed(|length| sealed::record_len(length).map(Vec::from));
        self.fetcher.fetch(plan, spool).await
    }
}

fn address(template: &str, index: usize) -> String {
    template.replace(SEGMENT, &index.to_string())
}
