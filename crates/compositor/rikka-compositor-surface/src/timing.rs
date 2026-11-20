use std::time::Duration;

use rikka_compositor_types::OutputId;

use crate::commit::drain;
use crate::constraints::Constraints;
use crate::host::SurfaceHost;
use crate::ids::SurfaceId;
use crate::surfaces::Surfaces;

pub fn refreshed<S: SurfaceHost>(state: &mut S, output: OutputId, next_presentation: Duration) {
    let surfaces = state.surfaces();
    match surfaces
        .horizons
        .iter_mut()
        .find(|(horizon_output, _)| *horizon_output == output)
    {
        Some((_, horizon)) => *horizon = next_presentation,
        None => surfaces.horizons.push((output, next_presentation)),
    }
    let mut barriers = std::mem::take(&mut surfaces.barriers);
    barriers.retain(|surface| {
        let Some(node) = surfaces.surfaces.get(*surface) else {
            return false;
        };
        if !node.current.barrier {
            return false;
        }
        if surfaces.main_output(*surface) != Some(output) {
            return true;
        }
        if let Some(node) = surfaces.surfaces.get_mut(*surface) {
            node.current.barrier = false;
        }
        false
    });
    surfaces.barriers = barriers;
    surfaces.queue.reconsider();
    drain(state);
}

impl Surfaces {
    pub fn set_main_output(&mut self, surface: SurfaceId, output: Option<OutputId>) {
        if let Some(surface) = self.surfaces.get_mut(surface)
            && surface.main_output != output
        {
            surface.main_output = output;
            self.queue.reconsider();
        }
    }

    pub fn main_output(&self, surface: SurfaceId) -> Option<OutputId> {
        let mut current = Some(surface);
        while let Some(node) = current.and_then(|id| self.surfaces.get(id)) {
            if node.main_output.is_some() {
                return node.main_output;
            }
            current = node.parent;
        }
        None
    }

    pub(crate) fn forget_horizon(&mut self, output: OutputId) {
        self.horizons
            .retain(|(horizon_output, _)| *horizon_output != output);
        for surface in self.surfaces.values_mut() {
            if surface.main_output == Some(output) {
                surface.main_output = None;
            }
        }
        self.queue.reconsider();
    }

    pub(crate) fn outputs_awaited(&mut self, now: Duration, outputs: &mut Vec<OutputId>) {
        outputs.clear();
        self.queue.prune_constrained();
        for transaction in self.queue.constrained() {
            for (surface, update) in &transaction.entries {
                if let Some(Some(output)) = self.unmet(*surface, &update.constraints, now) {
                    outputs.push(output);
                }
            }
        }
        outputs.sort_unstable();
        outputs.dedup();
    }

    pub(crate) fn unmet(
        &self,
        surface: SurfaceId,
        constraints: &Constraints,
        now: Duration,
    ) -> Option<Option<OutputId>> {
        if !constraints.wait_barrier && constraints.target.is_none_or(|target| target <= now) {
            return None;
        }
        let barrier = self
            .surfaces
            .get(surface)
            .is_some_and(|surface| surface.current.barrier);
        let output = self.main_output(surface);
        let fifo = constraints.wait_barrier && barrier && output.is_some();
        let early = constraints.target.is_some_and(|target| {
            target > now
                && output
                    .and_then(|output| self.horizon(output))
                    .is_none_or(|horizon| horizon < target)
        });
        (fifo || early).then_some(output)
    }

    fn horizon(&self, output: OutputId) -> Option<Duration> {
        self.horizons
            .iter()
            .find(|(horizon_output, _)| *horizon_output == output)
            .map(|(_, horizon)| *horizon)
    }
}
