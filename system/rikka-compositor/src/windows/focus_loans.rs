use rikka_compositor_seat::KeyboardFocus;
use rikka_compositor_surface::SurfaceId;

#[derive(Debug, Clone, Copy)]
struct Loan {
    to: SurfaceId,
    from: Option<KeyboardFocus>,
}

#[derive(Debug, Default)]
pub(crate) struct FocusLoans {
    loans: Vec<Loan>,
}

impl FocusLoans {
    pub(crate) fn lend(&mut self, to: SurfaceId, from: Option<KeyboardFocus>) {
        if from.is_some_and(|from| from.surface() == to) {
            return;
        }
        self.loans.retain(|loan| loan.to != to);
        self.loans.push(Loan { to, from });
    }

    pub(crate) fn repay(&mut self, to: SurfaceId) -> Option<KeyboardFocus> {
        let at = self.loans.iter().position(|loan| loan.to == to)?;
        let repaid = self.loans.remove(at);
        for loan in &mut self.loans {
            if loan.from.is_some_and(|from| from.surface() == to) {
                loan.from = repaid.from;
            }
        }
        repaid.from
    }
}
