use rikka_i18n::{Label, tr};
use rikka_llm::Effort;

#[must_use]
pub fn effort_label(effort: Effort) -> Label {
    match effort {
        Effort::Off => tr!("llm.effort.off"),
        Effort::Minimal => tr!("llm.effort.minimal"),
        Effort::Low => tr!("llm.effort.low"),
        Effort::Medium => tr!("llm.effort.medium"),
        Effort::High => tr!("llm.effort.high"),
        Effort::XHigh => tr!("llm.effort.xhigh"),
        Effort::Max => tr!("llm.effort.max"),
    }
}
