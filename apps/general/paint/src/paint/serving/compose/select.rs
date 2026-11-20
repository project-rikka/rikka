use paint_assistant::{Sample, SelectStep};
use paint_document::{Edit, Region, SelectionAction};

use super::{Composer, Refusal, stepped};

impl Composer<'_> {
    pub(super) fn select(&mut self, steps: Vec<SelectStep>) -> Result<(), Refusal> {
        for (at, step) in steps.into_iter().enumerate() {
            let kind = kind(&step);
            let applied = self.selected(step);
            stepped("step", at, kind, applied)?;
        }
        Ok(())
    }

    fn selected(&mut self, step: SelectStep) -> Result<(), Refusal> {
        let canvas = self.canvas();
        let edit = match step {
            SelectStep::Shape { shape, combine } => Edit::SelectShape { shape, combine },
            SelectStep::Wand { flood, combine } => Edit::MagicWand {
                flood: self.flood(&flood, Sample::Image, None)?,
                combine,
            },
            SelectStep::Layer { layer, combine } => Edit::SelectLayer {
                layer: self.layer(layer.as_ref())?.id,
                combine,
            },
            SelectStep::All => Edit::Select {
                region: Region::rectangle(canvas),
                by: SelectionAction::All,
            },
            SelectStep::Nothing => Edit::Select {
                region: Region::default(),
                by: SelectionAction::Deselect,
            },
            SelectStep::Invert => Edit::Select {
                region: self.document.selection().inverted(canvas),
                by: SelectionAction::Invert,
            },
            SelectStep::Modify(modify) => Edit::ModifySelection(modify),
            SelectStep::Move { dx, dy } => Edit::MoveSelection { dx, dy },
        };
        self.apply(&edit)
    }
}

fn kind(step: &SelectStep) -> &'static str {
    match step {
        SelectStep::Shape { .. } => "shape",
        SelectStep::Wand { .. } => "wand",
        SelectStep::Layer { .. } => "layer",
        SelectStep::All => "all",
        SelectStep::Nothing => "none",
        SelectStep::Invert => "invert",
        SelectStep::Modify(_) => "modify",
        SelectStep::Move { .. } => "move",
    }
}
