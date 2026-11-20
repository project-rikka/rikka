use std::hash::Hasher;
use std::sync::Arc;

use ash::vk;
use rustc_hash::{FxHashMap, FxHasher};

use crate::arena::{GpuArena, Span};
use crate::draw::Frame;
use crate::error::{Error, Resource, Result};
use crate::gpu::GpuMaterial;
use crate::material::MaterialId;
use crate::material::desc::MaterialDesc;
use crate::material::features::{self, Routing};
use crate::material::overrides::Override;
use crate::material::roles;
use crate::material::state::MaterialState;
use crate::material::template::{MaterialTemplate, Program};
use crate::material::value::{BLOCK_ALIGNMENT, Value, encode};
use crate::mirror::MirroredTable;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::texture::TextureId;

const INITIAL_RECORDS: u32 = 256;

const VARIANT_KEEP_FRAMES: u64 = 8;

type Applied = Vec<(usize, Value)>;

struct Record {
    index: u32,
    block: Span,
}

struct Variant {
    applied: Applied,
    record: Record,
    used: u64,
}

struct Entry {
    desc: MaterialDesc,
    base: Record,
    variants: FxHashMap<u64, Variant>,
}

struct Slot {
    generation: u32,
    entry: Option<Entry>,
}

struct RecordInfo {
    textures: Vec<u32>,
    routing: Routing,
    reach: Option<f32>,
}

pub(crate) struct Reload {
    pub template: MaterialTemplate,
    pub program: Arc<Program>,
    pub uses: Vec<(MaterialState, bool, Routing)>,
}

pub(crate) struct Warming {
    pub record: u32,
    pub routing: Routing,
    pub program: Arc<Program>,
    pub state: MaterialState,
    pub hull: bool,
}

impl Warming {
    pub(crate) fn used(&self) -> (MaterialState, bool, Routing) {
        (self.state, self.hull, self.routing)
    }
}

pub(crate) struct Drawn<'a> {
    pub desc: &'a MaterialDesc,
    pub record: u32,
    pub routing: Routing,
}

pub(crate) struct MaterialPool {
    table: MirroredTable<GpuMaterial>,
    records: Vec<RecordInfo>,
    warming: Vec<Warming>,
    blocks: GpuArena<1>,
    slots: Vec<Slot>,
    free_slots: Vec<u32>,
    resolved: Vec<u32>,
    applied: FxHashMap<(u64, u32), (u64, Applied)>,
    serial: u64,
    revision: u64,
}

impl MaterialPool {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Result<Self> {
        let table = MirroredTable::with_capacity(
            device.clone(),
            "rikka.three.materials",
            vk::PipelineStageFlags2::VERTEX_SHADER
                | vk::PipelineStageFlags2::FRAGMENT_SHADER
                | vk::PipelineStageFlags2::COMPUTE_SHADER,
            INITIAL_RECORDS,
        )?;
        let blocks = GpuArena::single(
            &device,
            "rikka.three.material_blocks",
            BLOCK_ALIGNMENT as u32,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        Ok(Self {
            table,
            records: Vec::new(),
            warming: Vec::new(),
            blocks,
            slots: Vec::new(),
            free_slots: Vec::new(),
            resolved: Vec::new(),
            applied: FxHashMap::default(),
            serial: 0,
            revision: 0,
        })
    }

    pub(crate) fn address(&self) -> vk::DeviceAddress {
        self.table.address()
    }

    pub(crate) fn blocks_address(&self) -> vk::DeviceAddress {
        self.blocks.address()
    }

    pub(crate) fn create(
        &mut self,
        desc: MaterialDesc,
        finished: u64,
        texture_slot: impl Fn(TextureId) -> Option<u32>,
    ) -> Result<MaterialId> {
        desc.state.validate()?;
        self.table.reclaim(finished);
        let base = self.record(&desc, &desc.values, &texture_slot)?;
        let entry = Entry {
            desc,
            base,
            variants: FxHashMap::default(),
        };
        let index = match self.free_slots.pop() {
            Some(index) => index as usize,
            None => {
                self.slots.push(Slot {
                    generation: 0,
                    entry: None,
                });
                self.slots.len() - 1
            }
        };
        let slot = &mut self.slots[index];
        slot.entry = Some(entry);
        Ok(MaterialId {
            index: index as u32,
            generation: slot.generation,
        })
    }

    fn record(
        &mut self,
        desc: &MaterialDesc,
        values: &[Value],
        texture_slot: &impl Fn(TextureId) -> Option<u32>,
    ) -> Result<Record> {
        let encoded = encode(&desc.program.schema, values, texture_slot)?;
        let block = self.blocks.insert([&encoded.bytes])?;
        let material = roles::record(&desc.program.schema, block.offset, desc.alpha_cutoff);
        let info = RecordInfo {
            textures: encoded.textures,
            routing: features::routing(&desc.program.schema, values),
            reach: displacement(desc, values),
        };
        let index = self.table.insert(|[record]| *record = material);
        match self.records.get_mut(index as usize) {
            Some(known) => *known = info,
            None => self.records.push(info),
        }
        Ok(Record { index, block })
    }

    fn retire(&mut self, record: Record, after: u64) {
        self.warming
            .retain(|warming| warming.record != record.index);
        self.blocks.retire(record.block);
        self.table.retire(record.index, after);
    }

    fn entry(&self, id: MaterialId) -> Option<&Entry> {
        let slot = self.slots.get(id.index as usize)?;
        match slot.generation == id.generation {
            true => slot.entry.as_ref(),
            false => None,
        }
    }

    fn entry_mut(&mut self, id: MaterialId) -> Option<&mut Entry> {
        let slot = self.slots.get_mut(id.index as usize)?;
        match slot.generation == id.generation {
            true => slot.entry.as_mut(),
            false => None,
        }
    }

    pub(crate) fn destroy(&mut self, id: MaterialId, after: u64) -> bool {
        let Some(slot) = self.slots.get_mut(id.index as usize) else {
            return false;
        };
        if slot.generation != id.generation {
            return false;
        }
        let Some(entry) = slot.entry.take() else {
            return false;
        };
        slot.generation = slot.generation.wrapping_add(1);
        self.free_slots.push(id.index);
        self.retire(entry.base, after);
        for (_, variant) in entry.variants {
            self.retire(variant.record, after);
        }
        true
    }

    pub(crate) fn set_values(
        &mut self,
        id: MaterialId,
        values: &[(&str, Value)],
        after: u64,
        texture_slot: impl Fn(TextureId) -> Option<u32>,
    ) -> Result<bool> {
        let Some(entry) = self.entry(id) else {
            return Err(Error::Unknown(Resource::Material(id)));
        };
        let mut desc = entry.desc.clone();
        for (name, value) in values {
            desc.set(name, value.clone())?;
        }
        if desc.values == entry.desc.values {
            return Ok(false);
        }
        self.replace(id, desc, after, &texture_slot)?;
        Ok(true)
    }

    fn replace(
        &mut self,
        id: MaterialId,
        desc: MaterialDesc,
        after: u64,
        texture_slot: &impl Fn(TextureId) -> Option<u32>,
    ) -> Result<()> {
        let base = self.record(&desc, &desc.values, texture_slot)?;
        let entry = self.entry_mut(id).expect("replaced material exists");
        entry.desc = desc;
        let previous = std::mem::replace(&mut entry.base, base);
        let variants = std::mem::take(&mut entry.variants);
        self.retire(previous, after);
        for (_, variant) in variants {
            self.retire(variant.record, after);
        }
        Ok(())
    }

    pub(crate) fn describe(&self, id: MaterialId) -> Option<&MaterialDesc> {
        self.entry(id).map(|entry| &entry.desc)
    }

    pub(crate) fn descs(&self) -> impl Iterator<Item = &MaterialDesc> {
        self.slots
            .iter()
            .filter_map(|slot| slot.entry.as_ref().map(|entry| &entry.desc))
    }

    pub(crate) fn reloads(&self) -> Vec<Reload> {
        let mut reloads: Vec<Reload> = Vec::new();
        for entry in self.slots.iter().filter_map(|slot| slot.entry.as_ref()) {
            let desc = &entry.desc;
            if desc.template.generation() == desc.program.generation {
                continue;
            }
            let program = desc.template.program();
            let reload = match reloads
                .iter_mut()
                .position(|reload| reload.template == desc.template)
            {
                Some(index) => &mut reloads[index],
                None => {
                    reloads.push(Reload {
                        template: desc.template.clone(),
                        program,
                        uses: Vec::new(),
                    });
                    reloads.last_mut().expect("just pushed")
                }
            };
            let used = (desc.state, desc.hull, features::of(desc));
            if !reload.uses.contains(&used) {
                reload.uses.push(used);
            }
        }
        reloads
    }

    pub(crate) fn adopt(
        &mut self,
        reload: &Reload,
        after: u64,
        texture_slot: impl Fn(TextureId) -> Option<u32>,
    ) -> Result<()> {
        let adopting: Vec<MaterialId> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                let entry = slot.entry.as_ref()?;
                (entry.desc.template == reload.template
                    && entry.desc.program.generation != reload.program.generation)
                    .then_some(MaterialId {
                        index: index as u32,
                        generation: slot.generation,
                    })
            })
            .collect();
        for id in adopting {
            let mut desc = self.describe(id).expect("adopting material exists").clone();
            desc.migrate(Arc::clone(&reload.program));
            self.replace(id, desc, after, &texture_slot)?;
        }
        Ok(())
    }

    pub(crate) fn resolve(
        &mut self,
        frame: &Frame,
        after: u64,
        texture_slot: impl Fn(TextureId) -> Option<u32>,
    ) -> Result<()> {
        self.serial += 1;
        let mut resolved = std::mem::take(&mut self.resolved);
        let mut applied = std::mem::take(&mut self.applied);
        resolved.clear();
        applied.clear();
        let result = self.resolve_into(frame, after, &texture_slot, &mut applied, &mut resolved);
        self.resolved = resolved;
        self.applied = applied;
        result?;
        self.evict(after);
        Ok(())
    }

    fn resolve_into(
        &mut self,
        frame: &Frame,
        after: u64,
        texture_slot: &impl Fn(TextureId) -> Option<u32>,
        applied: &mut FxHashMap<(u64, u32), (u64, Applied)>,
        resolved: &mut Vec<u32>,
    ) -> Result<()> {
        let serial = self.serial;
        for draw in &frame.draws {
            let Some(entry) = self.entry(draw.material) else {
                return Err(Error::Unknown(Resource::Material(draw.material)));
            };
            let figure = draw.figure.map_or(0, |figure| figure.0);
            let key = (entry.desc.program.generation, figure);
            let (hash, list) = match applied.entry(key) {
                std::collections::hash_map::Entry::Occupied(known) => known.into_mut(),
                std::collections::hash_map::Entry::Vacant(vacant) => {
                    let list = applicable(frame, figure, &entry.desc)?;
                    vacant.insert((hash_of(&list), list))
                }
            };
            if list.is_empty() {
                resolved.push(entry.base.index);
                continue;
            }
            let found = entry
                .variants
                .get(hash)
                .filter(|variant| variant.applied == *list)
                .map(|variant| variant.record.index);
            let record = match found {
                Some(record) => record,
                None => {
                    self.add_variant(draw.material, *hash, list.clone(), after, texture_slot)?
                }
            };
            if let Some(variant) = self
                .entry_mut(draw.material)
                .and_then(|entry| entry.variants.get_mut(hash))
            {
                variant.used = serial;
            }
            resolved.push(record);
        }
        Ok(())
    }

    fn add_variant(
        &mut self,
        id: MaterialId,
        hash: u64,
        applied: Applied,
        after: u64,
        texture_slot: &impl Fn(TextureId) -> Option<u32>,
    ) -> Result<u32> {
        let desc = self.describe(id).expect("varied material exists").clone();
        let mut values = desc.values.clone();
        for (index, value) in &applied {
            values[*index] = value.clone();
        }
        let record = self.record(&desc, &values, texture_slot)?;
        let index = record.index;
        let entry = self.entry_mut(id).expect("varied material exists");
        let base = entry.base.index;
        let previous = entry.variants.insert(
            hash,
            Variant {
                applied,
                record,
                used: 0,
            },
        );
        if let Some(previous) = previous {
            self.retire(previous.record, after);
        }
        let routing = self.records[index as usize].routing;
        let own = self.records[base as usize].routing;
        if routing != own {
            self.records[index as usize].routing = own;
            self.warming.push(Warming {
                record: index,
                routing,
                program: Arc::clone(&desc.program),
                state: desc.state,
                hull: desc.hull,
            });
        }
        Ok(index)
    }

    pub(crate) fn warming(&self) -> &[Warming] {
        &self.warming
    }

    pub(crate) fn settle(&mut self, record: u32) {
        if let Some(at) = self
            .warming
            .iter()
            .position(|warming| warming.record == record)
        {
            let warming = self.warming.swap_remove(at);
            self.records[record as usize].routing = warming.routing;
        }
    }

    fn evict(&mut self, after: u64) {
        let serial = self.serial;
        let mut stale = Vec::new();
        for slot in &mut self.slots {
            let Some(entry) = slot.entry.as_mut() else {
                continue;
            };
            entry.variants.retain(|_, variant| {
                let keep = variant.used + VARIANT_KEEP_FRAMES >= serial;
                if !keep {
                    stale.push(Record {
                        index: variant.record.index,
                        block: variant.record.block,
                    });
                }
                keep
            });
        }
        for record in stale {
            self.retire(record, after);
        }
    }

    pub(crate) fn drawn(&self, draw: usize, id: MaterialId) -> Option<Drawn<'_>> {
        let desc = &self.entry(id)?.desc;
        let record = *self.resolved.get(draw)?;
        Some(Drawn {
            desc,
            record,
            routing: self.records[record as usize].routing,
        })
    }

    pub(crate) fn reach(&self, record: u32) -> Option<f32> {
        self.records[record as usize].reach
    }

    pub(crate) fn texture_slots(&self, record: u32) -> impl Iterator<Item = u32> + '_ {
        self.records
            .get(record as usize)
            .into_iter()
            .flat_map(|info| info.textures.iter().copied())
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn flush(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<()> {
        if self.has_pending() {
            self.revision = self.revision.wrapping_add(1);
        }
        self.blocks.flush(ctx)?;
        self.table.flush(ctx)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.table.has_pending() || self.blocks.has_pending()
    }
}

fn applicable(frame: &Frame, figure: u32, desc: &MaterialDesc) -> Result<Applied> {
    let template = desc.template.name();
    let schema = &desc.program.schema;
    let look = frame
        .look
        .iter()
        .flat_map(|look| look.overrides.applying(template));
    let own = (figure > 0)
        .then(|| frame.figures.get(figure as usize - 1))
        .flatten()
        .into_iter()
        .flat_map(|figure| figure.overrides.applying(template));
    let mut list: Applied = Vec::new();
    for Override { name, value, .. } in look.chain(own) {
        let Some(index) = schema.index_of(name) else {
            continue;
        };
        value.check(&schema.parameters()[index])?;
        match list.iter_mut().find(|(known, _)| *known == index) {
            Some((_, existing)) => *existing = value.clone(),
            None => list.push((index, value.clone())),
        }
    }
    list.sort_by_key(|(index, _)| *index);
    Ok(list)
}

fn hash_of(list: &Applied) -> u64 {
    let mut hasher = FxHasher::default();
    for (index, value) in list {
        hasher.write_usize(*index);
        value.hash_into(&mut hasher);
    }
    hasher.finish()
}

fn displacement(desc: &MaterialDesc, values: &[Value]) -> Option<f32> {
    if !desc.program.deforms {
        return None;
    }
    let schema = &desc.program.schema;
    let index = schema
        .parameters()
        .iter()
        .position(|spec| spec.role == Some(rikka_slang::Role::Displacement))?;
    match values[index] {
        Value::Float(reach) if reach >= 0.0 && reach.is_finite() => Some(reach),
        _ => Some(f32::INFINITY),
    }
}
