#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum MaterialMode {
    #[default]
    IndexBased,
    Transfer,
}

pub fn material_remap<M: PartialEq + Clone>(
    mode: MaterialMode,
    result_slots: &mut Vec<Option<M>>,
    operand_slots: &[Option<M>],
) -> Vec<i32> {
    match mode {
        MaterialMode::IndexBased => index_based(result_slots, operand_slots),
        MaterialMode::Transfer => transfer(result_slots, operand_slots),
    }
}

fn index_based<M: PartialEq>(target: &[Option<M>], source: &[Option<M>]) -> Vec<i32> {
    if source.is_empty() {
        return vec![0];
    }
    source
        .iter()
        .enumerate()
        .map(|(slot, material)| {
            if target.get(slot) == Some(material) {
                return slot as i32;
            }
            match target.iter().position(|candidate| candidate == material) {
                Some(found) => found as i32,
                None if slot < target.len() => slot as i32,
                None => 0,
            }
        })
        .collect()
}

fn transfer<M: PartialEq + Clone>(result: &mut Vec<Option<M>>, source: &[Option<M>]) -> Vec<i32> {
    if result.is_empty() {
        result.push(None);
    }
    source
        .iter()
        .map(|material| match material {
            None => -1,
            Some(_) => match result.iter().position(|candidate| candidate == material) {
                Some(found) => found as i32,
                None => {
                    result.push(material.clone());
                    (result.len() - 1) as i32
                }
            },
        })
        .collect()
}
