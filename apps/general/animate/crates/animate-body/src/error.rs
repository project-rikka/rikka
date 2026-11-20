use rikka_model_core::HumanBone;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("missing humanoid bones: {}", listed(.missing))]
pub struct MappingError {
    pub missing: Vec<HumanBone>,
}

fn listed(missing: &[HumanBone]) -> String {
    match missing {
        [] => "a human body".to_owned(),
        _ => missing
            .iter()
            .map(|bone| bone.name())
            .collect::<Vec<_>>()
            .join(", "),
    }
}
