use rikka_gpu::Ticket;

use crate::cloth::Cloth;
use crate::figure::FigureId;

#[must_use = "recorded drapes are visible only after their batch is submitted and completed"]
#[derive(Debug)]
pub struct Recorded {
    pub(crate) shown: Vec<FigureId>,
}

impl Cloth {
    pub fn commit(&mut self, recorded: Recorded, ready: &Ticket) {
        for id in &recorded.shown {
            if let Some(outputs) = self
                .figures
                .get_mut(id)
                .and_then(|figure| figure.outputs.as_mut())
            {
                outputs.present(ready.clone());
            }
        }
    }
}
