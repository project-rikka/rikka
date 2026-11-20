use crate::instruction::instructions;

const OP_DECORATE: u32 = 71;
const DECORATION_BUILT_IN: u32 = 11;
const DECORATION_INVARIANT: u32 = 18;
const BUILT_IN_POSITION: u32 = 0;

pub fn invariant_position(words: &mut Vec<u32>) -> bool {
    let Ok(module) = instructions(words) else {
        return false;
    };
    let mut position = None;
    let mut invariant = Vec::new();
    for instruction in module {
        let Ok(instruction) = instruction else {
            return false;
        };
        if instruction.opcode != OP_DECORATE {
            continue;
        }
        match *instruction.operands {
            [target, DECORATION_BUILT_IN, BUILT_IN_POSITION] => {
                position.get_or_insert((instruction.end, target));
            }
            [target, DECORATION_INVARIANT] => invariant.push(target),
            _ => {}
        }
    }
    let Some((end, target)) = position else {
        return false;
    };
    if !invariant.contains(&target) {
        words.splice(
            end..end,
            [(3 << 16) | OP_DECORATE, target, DECORATION_INVARIANT],
        );
    }
    true
}
