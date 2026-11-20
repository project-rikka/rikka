use std::sync::Arc;

use ash::vk;
use rikka_sync::Mutex;

use crate::core::device::RenderDevice;
use crate::core::image::{GpuImage, ImageAccess};
use crate::core::placed_slots::{PLACED_SLOTS, PlacedSlots};
use crate::core::sampler::Samplers;
use crate::error::{Error, Result, Store};

pub(crate) const BINDING_TEXTURES: u32 = 0;
pub(crate) const BINDING_IMAGES: u32 = 1;
pub(crate) const BINDING_SAMPLERS: u32 = 2;
pub(crate) const BINDING_STORAGE_IMAGES: u32 = 3;
pub(crate) const BINDING_PLACED_EXTENTS: u32 = 4;

const WRITE_BATCH: usize = 32;

struct SlotAllocator {
    what: &'static str,
    capacity: u32,
    next: u32,
    free: Vec<u32>,
    pending: Vec<(u64, u32)>,
}

impl SlotAllocator {
    fn new(what: &'static str, capacity: u32) -> Self {
        Self::reserving(what, capacity, 0)
    }

    fn reserving(what: &'static str, capacity: u32, reserved: u32) -> Self {
        Self {
            what,
            capacity,
            next: reserved,
            free: Vec::new(),
            pending: Vec::new(),
        }
    }

    fn allocate(&mut self) -> Result<u32> {
        if let Some(slot) = self.free.pop() {
            return Ok(slot);
        }
        if self.next >= self.capacity {
            return Err(Error::Full(Store::Table {
                what: self.what,
                slots: self.capacity,
            }));
        }
        let slot = self.next;
        self.next += 1;
        Ok(slot)
    }

    fn release(&mut self, slot: u32, retire: u64) {
        self.pending.push((retire, slot));
    }

    fn collect(&mut self, completed: u64) {
        let free = &mut self.free;
        self.pending.retain(|&(retire, slot)| {
            if retire <= completed {
                free.push(slot);
                false
            } else {
                true
            }
        });
    }
}

#[derive(Clone)]
pub struct TextureSlots(Arc<Mutex<SlotAllocator>>);

impl TextureSlots {
    pub(crate) fn reserve(&self) -> Result<u32> {
        self.0.lock().allocate()
    }

    fn capacity(&self) -> u32 {
        self.0.lock().capacity
    }
}

pub struct ResourceTable {
    device: Arc<RenderDevice>,
    layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    textures: TextureSlots,
    storage_images: SlotAllocator,
    placed: Option<PlacedSlots>,
    fallback: GpuImage,
    fallback_storage: Option<GpuImage>,
    recycled: Vec<u32>,
}

struct TableShape {
    bindings: Vec<vk::DescriptorSetLayoutBinding<'static>>,
    binding_flags: Vec<vk::DescriptorBindingFlags>,
    pool_sizes: Vec<vk::DescriptorPoolSize>,
}

impl TableShape {
    fn new(capacity: u32, storage_capacity: u32, sampler_count: u32) -> TableShape {
        let indexed = vk::DescriptorBindingFlags::PARTIALLY_BOUND
            | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND
            | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING;
        let stages = vk::ShaderStageFlags::VERTEX
            | vk::ShaderStageFlags::FRAGMENT
            | vk::ShaderStageFlags::COMPUTE;
        let binding = |binding: u32, ty: vk::DescriptorType, count: u32| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(binding)
                .descriptor_type(ty)
                .descriptor_count(count)
                .stage_flags(stages)
        };
        let pool_size = |ty: vk::DescriptorType, descriptor_count: u32| vk::DescriptorPoolSize {
            ty,
            descriptor_count,
        };
        let mut shape = TableShape {
            bindings: vec![
                binding(
                    BINDING_TEXTURES,
                    vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    capacity,
                ),
                binding(BINDING_IMAGES, vk::DescriptorType::SAMPLED_IMAGE, capacity),
                binding(BINDING_SAMPLERS, vk::DescriptorType::SAMPLER, sampler_count),
            ],
            binding_flags: vec![indexed, indexed, vk::DescriptorBindingFlags::empty()],
            pool_sizes: vec![
                pool_size(vk::DescriptorType::COMBINED_IMAGE_SAMPLER, capacity),
                pool_size(vk::DescriptorType::SAMPLED_IMAGE, capacity),
                pool_size(vk::DescriptorType::SAMPLER, sampler_count),
            ],
        };
        if storage_capacity > 0 {
            shape.binding_flags.push(indexed);
            shape.bindings.push(binding(
                BINDING_STORAGE_IMAGES,
                vk::DescriptorType::STORAGE_IMAGE,
                storage_capacity,
            ));
            shape.pool_sizes.push(pool_size(
                vk::DescriptorType::STORAGE_IMAGE,
                storage_capacity,
            ));
            shape
                .binding_flags
                .push(vk::DescriptorBindingFlags::empty());
            shape.bindings.push(binding(
                BINDING_PLACED_EXTENTS,
                vk::DescriptorType::STORAGE_BUFFER,
                1,
            ));
            shape
                .pool_sizes
                .push(pool_size(vk::DescriptorType::STORAGE_BUFFER, 1));
        }
        shape
    }
}

fn fallback_images(
    device: &Arc<RenderDevice>,
    storage: bool,
) -> Result<(GpuImage, Option<GpuImage>)> {
    let single = vk::Extent2D {
        width: 1,
        height: 1,
    };
    let fallback = GpuImage::new(
        device.clone(),
        "rikka.fallback",
        single,
        vk::Format::R8G8B8A8_UNORM,
        vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
    )?;
    let fallback_storage = match storage {
        true => Some(GpuImage::new(
            device.clone(),
            "rikka.fallback_storage",
            single,
            vk::Format::R32G32B32A32_SFLOAT,
            vk::ImageUsageFlags::STORAGE,
        )?),
        false => None,
    };
    Ok((fallback, fallback_storage))
}

fn create_table(
    device: &RenderDevice,
    shape: &TableShape,
) -> Result<(
    vk::DescriptorSetLayout,
    vk::DescriptorPool,
    vk::DescriptorSet,
)> {
    let handle = device.handle();
    let mut flags_info = vk::DescriptorSetLayoutBindingFlagsCreateInfo::default()
        .binding_flags(&shape.binding_flags);
    let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
        .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
        .bindings(&shape.bindings)
        .push_next(&mut flags_info);
    let layout = unsafe { handle.create_descriptor_set_layout(&layout_info, None)? };

    let pool_info = vk::DescriptorPoolCreateInfo::default()
        .flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND)
        .max_sets(1)
        .pool_sizes(&shape.pool_sizes);
    let pool = match unsafe { handle.create_descriptor_pool(&pool_info, None) } {
        Ok(pool) => pool,
        Err(err) => {
            unsafe { handle.destroy_descriptor_set_layout(layout, None) };
            return Err(err.into());
        }
    };

    let layouts = [layout];
    let alloc_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(pool)
        .set_layouts(&layouts);
    match unsafe { handle.allocate_descriptor_sets(&alloc_info) } {
        Ok(sets) => Ok((layout, pool, sets[0])),
        Err(err) => {
            unsafe {
                handle.destroy_descriptor_pool(pool, None);
                handle.destroy_descriptor_set_layout(layout, None);
            }
            Err(err.into())
        }
    }
}

impl ResourceTable {
    pub(crate) fn new(device: Arc<RenderDevice>, samplers: &Samplers) -> Result<Self> {
        let capacity = device.table_capacity();
        let storage_capacity = device.storage_image_capacity();
        let (fallback, fallback_storage) = fallback_images(&device, storage_capacity > 0)?;
        let shape = TableShape::new(capacity, storage_capacity, samplers.table().len() as u32);
        let (layout, pool, set) = create_table(&device, &shape)?;
        device.set_object_name(set, "rikka.resource_table");

        let placed = match storage_capacity > 0 {
            true => Some(PlacedSlots::new(device.clone())?),
            false => None,
        };
        let reserved = match placed {
            Some(_) => PLACED_SLOTS,
            None => 0,
        };
        let table = Self {
            device,
            layout,
            pool,
            set,
            textures: TextureSlots(Arc::new(Mutex::new(SlotAllocator::reserving(
                "texture", capacity, reserved,
            )))),
            storage_images: SlotAllocator::new("storage image", storage_capacity),
            placed,
            fallback,
            fallback_storage,
            recycled: Vec::new(),
        };
        table.fill_fallback(samplers);
        Ok(table)
    }

    fn fill_fallback(&self, samplers: &Samplers) {
        let combined: Vec<vk::DescriptorImageInfo> = vec![
            vk::DescriptorImageInfo {
                sampler: samplers.linear_clamp(),
                image_view: self.fallback.view(),
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            };
            self.textures.capacity() as usize
        ];
        let images: Vec<vk::DescriptorImageInfo> = vec![
            vk::DescriptorImageInfo {
                sampler: vk::Sampler::null(),
                image_view: self.fallback.view(),
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            };
            self.textures.capacity() as usize
        ];
        let sampler_infos: Vec<vk::DescriptorImageInfo> = samplers
            .table()
            .iter()
            .map(|&sampler| vk::DescriptorImageInfo {
                sampler,
                image_view: vk::ImageView::null(),
                image_layout: vk::ImageLayout::UNDEFINED,
            })
            .collect();

        let mut writes = vec![
            vk::WriteDescriptorSet::default()
                .dst_set(self.set)
                .dst_binding(BINDING_TEXTURES)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&combined),
            vk::WriteDescriptorSet::default()
                .dst_set(self.set)
                .dst_binding(BINDING_IMAGES)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&images),
            vk::WriteDescriptorSet::default()
                .dst_set(self.set)
                .dst_binding(BINDING_SAMPLERS)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .image_info(&sampler_infos),
        ];

        let storage: Vec<vk::DescriptorImageInfo> = match &self.fallback_storage {
            Some(image) => vec![
                vk::DescriptorImageInfo {
                    sampler: vk::Sampler::null(),
                    image_view: image.view(),
                    image_layout: vk::ImageLayout::GENERAL,
                };
                self.storage_images.capacity as usize
            ],
            None => Vec::new(),
        };
        if !storage.is_empty() {
            writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(self.set)
                    .dst_binding(BINDING_STORAGE_IMAGES)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                    .image_info(&storage),
            );
        }
        let placed = self.placed.as_ref().map(|placed| {
            [vk::DescriptorBufferInfo {
                buffer: placed.buffer(),
                offset: 0,
                range: vk::WHOLE_SIZE,
            }]
        });
        if let Some(placed) = &placed {
            writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(self.set)
                    .dst_binding(BINDING_PLACED_EXTENTS)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(placed),
            );
        }

        unsafe { self.device.handle().update_descriptor_sets(&writes, &[]) };
    }

    pub(crate) fn prepare(&mut self, cmd: vk::CommandBuffer, frame_slot: usize) {
        if let Some(placed) = &mut self.placed {
            placed.begin(frame_slot);
        }
        if self.fallback.access() == ImageAccess::TABLE_SAMPLED {
            return;
        }
        self.fallback.barrier_to(cmd, ImageAccess::TRANSFER_DST);
        let clear = vk::ClearColorValue {
            float32: [1.0, 0.0, 1.0, 1.0],
        };
        let ranges = [crate::core::image::color_subresource_range()];
        unsafe {
            self.device.handle().cmd_clear_color_image(
                cmd,
                self.fallback.handle(),
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &clear,
                &ranges,
            );
        }
        self.fallback.barrier_to(cmd, ImageAccess::TABLE_SAMPLED);

        if let Some(storage) = &mut self.fallback_storage {
            storage.barrier_to(cmd, ImageAccess::COMPUTE_STORAGE);
        }
    }

    pub(crate) fn layout(&self) -> vk::DescriptorSetLayout {
        self.layout
    }

    pub fn set(&self) -> vk::DescriptorSet {
        self.set
    }

    pub(crate) fn texture_slots(&self) -> TextureSlots {
        self.textures.clone()
    }

    pub fn allocate(&mut self, view: vk::ImageView, sampler: vk::Sampler) -> Result<u32> {
        self.allocate_in(view, sampler, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
    }

    pub(crate) fn bind_reserved(&mut self, slot: u32, view: vk::ImageView, sampler: vk::Sampler) {
        self.write_textures(
            &[slot],
            view,
            sampler,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
    }

    pub fn allocate_in(
        &mut self,
        view: vk::ImageView,
        sampler: vk::Sampler,
        layout: vk::ImageLayout,
    ) -> Result<u32> {
        let slot = self.textures.reserve()?;
        self.write_textures(&[slot], view, sampler, layout);
        Ok(slot)
    }

    pub fn place(
        &mut self,
        view: vk::ImageView,
        sampler: vk::Sampler,
        layout: vk::ImageLayout,
        extent: vk::Extent2D,
    ) -> Result<u32> {
        let slot = self
            .placed
            .as_mut()
            .expect("placed slots exist with 3D support")
            .take(extent)?;
        self.write_textures(&[slot], view, sampler, layout);
        Ok(slot)
    }

    fn write_textures(
        &self,
        slots: &[u32],
        view: vk::ImageView,
        sampler: vk::Sampler,
        image_layout: vk::ImageLayout,
    ) {
        let combined = [vk::DescriptorImageInfo {
            sampler,
            image_view: view,
            image_layout,
        }];
        let plain = [vk::DescriptorImageInfo {
            sampler: vk::Sampler::null(),
            image_view: view,
            image_layout,
        }];
        let mut writes = [vk::WriteDescriptorSet::default(); WRITE_BATCH * 2];
        for chunk in slots.chunks(WRITE_BATCH) {
            let (pairs, _) = writes.as_chunks_mut::<2>();
            for (pair, &slot) in pairs.iter_mut().zip(chunk) {
                *pair = [
                    vk::WriteDescriptorSet::default()
                        .dst_set(self.set)
                        .dst_binding(BINDING_TEXTURES)
                        .dst_array_element(slot)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&combined),
                    vk::WriteDescriptorSet::default()
                        .dst_set(self.set)
                        .dst_binding(BINDING_IMAGES)
                        .dst_array_element(slot)
                        .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                        .image_info(&plain),
                ];
            }
            unsafe {
                self.device
                    .handle()
                    .update_descriptor_sets(&writes[..chunk.len() * 2], &[]);
            }
        }
    }

    pub fn allocate_storage(&mut self, view: vk::ImageView) -> Result<u32> {
        let slot = self.storage_images.allocate()?;
        self.write_storages(&[slot], view);
        Ok(slot)
    }

    fn write_storages(&self, slots: &[u32], view: vk::ImageView) {
        let info = [vk::DescriptorImageInfo {
            sampler: vk::Sampler::null(),
            image_view: view,
            image_layout: vk::ImageLayout::GENERAL,
        }];
        let mut writes = [vk::WriteDescriptorSet::default(); WRITE_BATCH];
        for chunk in slots.chunks(WRITE_BATCH) {
            for (write, &slot) in writes.iter_mut().zip(chunk) {
                *write = vk::WriteDescriptorSet::default()
                    .dst_set(self.set)
                    .dst_binding(BINDING_STORAGE_IMAGES)
                    .dst_array_element(slot)
                    .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                    .image_info(&info);
            }
            unsafe {
                self.device
                    .handle()
                    .update_descriptor_sets(&writes[..chunk.len()], &[]);
            }
        }
    }

    pub fn release(&mut self, slot: u32, retire: u64) {
        self.textures.0.lock().release(slot, retire);
    }

    pub fn release_storage(&mut self, slot: u32, retire: u64) {
        self.storage_images.release(slot, retire);
    }

    pub(crate) fn collect(&mut self, completed: u64, samplers: &Samplers) {
        self.recycled.clear();
        {
            let mut textures = self.textures.0.lock();
            let before = textures.free.len();
            textures.collect(completed);
            self.recycled.extend_from_slice(&textures.free[before..]);
        }
        self.write_textures(
            &self.recycled,
            self.fallback.view(),
            samplers.linear_clamp(),
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );

        if let Some(view) = self.fallback_storage.as_ref().map(GpuImage::view) {
            let before = self.storage_images.free.len();
            self.storage_images.collect(completed);
            self.write_storages(&self.storage_images.free[before..], view);
        }
    }
}

impl Drop for ResourceTable {
    fn drop(&mut self) {
        unsafe {
            self.device
                .handle()
                .destroy_descriptor_pool(self.pool, None);
            self.device
                .handle()
                .destroy_descriptor_set_layout(self.layout, None);
        }
    }
}

impl std::fmt::Debug for TextureSlots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextureSlots").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ResourceTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceTable").finish_non_exhaustive()
    }
}
