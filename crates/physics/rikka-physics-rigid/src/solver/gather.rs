use rikka_physics_core::{Block, Shared, Stage, prefetch};

use crate::body::NONE;
use crate::contact::SKIP;
use crate::graph::{COLOR_COUNT, OVERFLOW};
use crate::joint::JointKind;
use crate::solver::contact::encode;
use crate::world::RigidWorld;

const GATHER_BLOCK: usize = 256;
const GATHER_PREFETCH: usize = 8;
const SKIPPED: u32 = u32::MAX;
const EXTRA: u32 = 4;

#[derive(Debug, Default)]
pub(crate) struct ColorGather {
    contact_offsets: [usize; COLOR_COUNT + 1],
    joint_offsets: [usize; COLOR_COUNT + 1],
    contact_codes: Vec<u32>,
    joint_codes: Vec<(u32, u32)>,
    pub by_color: Vec<Vec<u32>>,
    pub joints_by_color: Vec<Vec<(u32, u32, u32)>>,
}

impl RigidWorld {
    pub(crate) fn gather_colors(&self, gather: &mut ColorGather, task_of: &[u32]) -> bool {
        let colors = &self.graph.colors;
        for (color, slot) in colors.iter().enumerate() {
            gather.contact_offsets[color + 1] = gather.contact_offsets[color] + slot.contacts.len();
            gather.joint_offsets[color + 1] = gather.joint_offsets[color] + slot.joints.len();
        }
        let contact_total = gather.contact_offsets[COLOR_COUNT];
        let joint_total = gather.joint_offsets[COLOR_COUNT];
        gather.contact_codes.resize(contact_total, SKIPPED);
        gather.joint_codes.resize(joint_total, (SKIPPED, 0));

        let contact_blocks = contact_total.div_ceil(GATHER_BLOCK);
        let joint_blocks = joint_total.div_ceil(GATHER_BLOCK);
        let contact_codes = Shared::new(&mut gather.contact_codes);
        let joint_codes = Shared::new(&mut gather.joint_codes);
        let (contact_offsets, joint_offsets) = (&gather.contact_offsets, &gather.joint_offsets);
        let stamp = self.narrowphase.stamp();
        let task = |block: Block| {
            let index = block.index as usize;
            if index < contact_blocks {
                let range = (Block {
                    index: index as u32,
                    ..block
                })
                .range(GATHER_BLOCK, contact_total);
                let mut color =
                    contact_offsets.partition_point(|&offset| offset <= range.start) - 1;
                for flat in range {
                    while contact_offsets[color + 1] <= flat {
                        color += 1;
                    }
                    let ids = &colors[color].contacts;
                    let local = flat - contact_offsets[color];
                    if let Some(&ahead) = ids.get(local + GATHER_PREFETCH) {
                        prefetch(self.contacts.get(ahead), 64);
                        prefetch(&self.headers[ahead as usize], 64);
                    }
                    let contact = ids[local];
                    let record = self.contacts.get(contact);
                    let code = if record.flags & SKIP != 0 || !colored(task_of, record.island) {
                        SKIPPED
                    } else {
                        let header = &self.headers[contact as usize];
                        if header.extra.stamp == stamp && header.extra.count > 0 {
                            EXTRA + header.extra.count
                        } else {
                            4 - u32::from(header.points).clamp(1, 4)
                        }
                    };
                    *unsafe { contact_codes.get(flat) } = code;
                }
                return;
            }
            let range = (Block {
                index: (index - contact_blocks) as u32,
                ..block
            })
            .range(GATHER_BLOCK, joint_total);
            let mut color = joint_offsets.partition_point(|&offset| offset <= range.start) - 1;
            for flat in range {
                while joint_offsets[color + 1] <= flat {
                    color += 1;
                }
                let joint = self
                    .joints
                    .get(colors[color].joints[flat - joint_offsets[color]]);
                let kind = joint.params.kind();
                *unsafe { joint_codes.get(flat) } =
                    if kind != JointKind::Filter && colored(task_of, joint.island) {
                        (kind.index() as u32, joint.params.layout())
                    } else {
                        (SKIPPED, 0)
                    };
            }
        };
        self.executor.run(
            &[Stage::new(
                "plan gather",
                (contact_blocks + joint_blocks) as u32,
            )],
            &task,
        );

        gather.by_color.resize_with(COLOR_COUNT * 4, Vec::new);
        gather.joints_by_color.resize_with(COLOR_COUNT, Vec::new);
        for list in &mut gather.by_color {
            list.clear();
        }
        let mut volatile = false;
        for (color, slot) in colors.iter().enumerate() {
            let codes = &gather.contact_codes
                [gather.contact_offsets[color]..gather.contact_offsets[color + 1]];
            for (&contact, &code) in slot.contacts.iter().zip(codes) {
                match code {
                    SKIPPED => {}
                    EXTRA.. => {
                        volatile = true;
                        gather.by_color[OVERFLOW as usize * 4]
                            .extend((0..=code - EXTRA).map(|slot| encode(contact, slot)));
                    }
                    bucket => gather.by_color[color * 4 + bucket as usize].push(contact),
                }
            }
            let codes =
                &gather.joint_codes[gather.joint_offsets[color]..gather.joint_offsets[color + 1]];
            let joints = &mut gather.joints_by_color[color];
            joints.clear();
            joints.extend(
                slot.joints
                    .iter()
                    .zip(codes)
                    .filter(|(_, code)| code.0 != SKIPPED)
                    .map(|(&joint, &(kind, layout))| (kind, layout, joint)),
            );
        }
        volatile
    }
}

#[inline]
fn colored(task_of: &[u32], island: u32) -> bool {
    island != NONE && task_of[island as usize] == NONE
}
