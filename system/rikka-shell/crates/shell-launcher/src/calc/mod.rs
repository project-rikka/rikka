mod worth;

use std::time::{Duration, Instant};

use rikka_ui::{MaterialSymbol, tr};

use crate::provider::{Activation, CERTAIN, Hit, ProviderInfo, Query, QueryToken, RemoteSearch};

pub(crate) const ID: &str = "calc";

const TRIGGER: &str = "=";

const DEADLINE: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub(super) struct CalcProvider {
    context: fend_core::Context,
}

impl CalcProvider {
    fn evaluate(&mut self, expression: &str, token: &QueryToken) -> Option<String> {
        let interrupt = DeadlineInterrupt {
            deadline: Instant::now() + DEADLINE,
            token: token.clone(),
        };
        let answer =
            fend_core::evaluate_with_interrupt(expression, &mut self.context, &interrupt).ok()?;
        if answer.output_is_empty() {
            return None;
        }
        let result = answer.get_main_result().trim().to_owned();
        (!result.is_empty() && result != expression.trim()).then_some(result)
    }
}

impl RemoteSearch for CalcProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo::new(ID, tr!("shell.launcher.provider.calculator"))
            .icon(MaterialSymbol::CALCULATE)
            .trigger(TRIGGER)
            .without_history()
    }

    fn search(&mut self, query: &Query, token: &QueryToken) -> Vec<Hit> {
        let expression = query.text();
        if expression.is_empty() || !(query.is_forced() || worth::evaluating(expression)) {
            return Vec::new();
        }
        let Some(result) = self.evaluate(expression, token) else {
            return Vec::new();
        };
        vec![
            Hit::new(result.clone(), result)
                .subtitle(expression)
                .score(CERTAIN),
        ]
    }

    fn taken(hit: &Hit) -> Activation {
        Activation::Copy(hit.id().to_owned())
    }
}

struct DeadlineInterrupt {
    deadline: Instant,
    token: QueryToken,
}

impl fend_core::Interrupt for DeadlineInterrupt {
    fn should_interrupt(&self) -> bool {
        self.token.is_stale() || Instant::now() >= self.deadline
    }
}
