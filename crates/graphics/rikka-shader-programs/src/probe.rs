use std::collections::{HashMap, HashSet};

use rikka_slang::Program;
use rikka_spirv::{instructions, literal_string};

const OP_NAME: u32 = 5;
const OP_TYPE_POINTER: u32 = 32;
const OP_FUNCTION: u32 = 54;
const OP_FUNCTION_PARAMETER: u32 = 55;
const STORAGE_FUNCTION: u32 = 7;

const HOOKS: [&str; 17] = [
    "evaluate",
    "coverage",
    "transmittance",
    "hull",
    "deform",
    "prepare",
    "direct",
    "transmit",
    "indirect",
    "resolve",
    "resolve_hull",
    "shade",
    "hull_scale",
    "seen_through",
    "gbuffer",
    "emitted",
    "scattered",
];

pub(crate) fn copied_methods(probe: &Program, owners: [&str; 2]) -> Vec<String> {
    let mut copied = Vec::new();
    for entry in probe.entry_points() {
        let Ok(module) = instructions(&entry.spirv) else {
            continue;
        };
        let mut names = HashMap::new();
        let mut local_pointers = HashSet::new();
        let mut functions = Vec::new();
        let mut current = None;
        for instruction in module.map_while(Result::ok) {
            match (instruction.opcode, instruction.operands) {
                (OP_NAME, [target, text @ ..]) => {
                    names.insert(*target, literal_string(text).0);
                }
                (OP_TYPE_POINTER, [pointer, STORAGE_FUNCTION, ..]) => {
                    local_pointers.insert(*pointer);
                }
                (OP_FUNCTION, [_, function, ..]) => current = Some(*function),
                (OP_FUNCTION_PARAMETER, [ty, ..]) => {
                    if let Some(function) = current.take()
                        && local_pointers.contains(ty)
                    {
                        functions.push(function);
                    }
                }
                _ => {}
            }
        }
        for function in functions {
            let Some(name) = names.get(&function) else {
                continue;
            };
            let Some((owner, method)) = name.rsplit_once('.') else {
                continue;
            };
            if owners.contains(&owner) && HOOKS.contains(&method) && !copied.contains(name) {
                copied.push(name.clone());
            }
        }
    }
    copied
}
