use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ash::vk;
use rikka_gpu::{
    ExternalVideoFrame, FrameClaim, LentFrame, VideoDeviceHandles, VideoFrameImage, VideoPlane,
    ycbcr_matrix,
};
use rikka_renderer::{Node, OffscreenTarget, Scene, TextureId};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIZE: u32 = 64;
const LUMA: f32 = 0.6;
const CHROMA: [f32; 2] = [0.3, 0.7];

struct Plane {
    image: vk::Image,
    memory: vk::DeviceMemory,
}

const DECODED: u64 = 1;
const PATIENCE: Duration = Duration::from_secs(5);
const RETURN_POLL: Duration = Duration::from_millis(5);
const CONVERTER_COMPILE_POLLS: usize = 200;
const CONVERTER_COMPILE_POLL: Duration = Duration::from_millis(10);

struct FrameState {
    value: u64,
    layout: vk::ImageLayout,
}

struct DecodedFrame {
    semaphore: vk::Semaphore,
    images: [vk::Image; 2],
    state: Arc<Mutex<FrameState>>,
    returned: Arc<AtomicBool>,
}

impl LentFrame for DecodedFrame {
    fn claim(&mut self) -> FrameClaim {
        let mut state = self.state.lock().expect("frame state");
        let claimed = FrameClaim {
            images: self
                .images
                .iter()
                .map(|&image| VideoFrameImage {
                    image,
                    layout: state.layout,
                    source_queue_family: None,
                })
                .collect(),
            waits: vec![(self.semaphore, state.value)],
            signals: vec![(self.semaphore, state.value + 1)],
        };
        state.value += 1;
        state.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        claimed
    }
}

impl Drop for DecodedFrame {
    fn drop(&mut self) {
        self.returned.store(true, Ordering::Release);
    }
}

struct Lent {
    semaphore: vk::Semaphore,
    state: Arc<Mutex<FrameState>>,
    returned: Arc<AtomicBool>,
}

struct Decoder {
    handles: VideoDeviceHandles,
    device: ash::Device,
    queue: vk::Queue,
    pool: vk::CommandPool,
    semaphores: Vec<vk::Semaphore>,
    lent: Vec<Lent>,
    planes: Vec<Plane>,
    memory: vk::PhysicalDeviceMemoryProperties,
}

impl Decoder {
    fn new(renderer: &TestRenderer) -> Option<Self> {
        let Some(handles) = renderer.uploads().video_device() else {
            eprintln!("this GPU decodes no video; skipping");
            return None;
        };
        let instance = unsafe { ash::Instance::load(handles.entry.static_fn(), handles.instance) };
        let device = unsafe { ash::Device::load(instance.fp_v1_0(), handles.device) };
        let memory =
            unsafe { instance.get_physical_device_memory_properties(handles.physical_device) };
        let family = handles.graphics_queue_family;
        let queue = unsafe { device.get_device_queue(family, 0) };
        let pool = unsafe {
            device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default().queue_family_index(family),
                    None,
                )
                .expect("a command pool")
        };
        Some(Self {
            handles,
            device,
            queue,
            pool,
            semaphores: Vec::new(),
            lent: Vec::new(),
            planes: Vec::new(),
            memory,
        })
    }

    fn timeline(&mut self) -> vk::Semaphore {
        let mut timeline = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);
        let semaphore = unsafe {
            self.device
                .create_semaphore(
                    &vk::SemaphoreCreateInfo::default().push_next(&mut timeline),
                    None,
                )
                .expect("a timeline semaphore")
        };
        self.semaphores.push(semaphore);
        semaphore
    }

    fn plane(&mut self, format: vk::Format, extent: u32) -> vk::Image {
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D {
                width: extent,
                height: extent,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        unsafe {
            let image = self
                .device
                .create_image(&info, None)
                .expect("a plane image");
            let requirements = self.device.get_image_memory_requirements(image);
            let memory_type = (0..self.memory.memory_type_count)
                .find(|&index| {
                    requirements.memory_type_bits & (1 << index) != 0
                        && self.memory.memory_types[index as usize]
                            .property_flags
                            .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
                })
                .expect("device-local memory");
            let memory = self
                .device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(requirements.size)
                        .memory_type_index(memory_type),
                    None,
                )
                .expect("plane memory");
            self.device
                .bind_image_memory(image, memory, 0)
                .expect("plane memory binds");
            self.planes.push(Plane { image, memory });
            image
        }
    }

    fn decode(&mut self) -> ExternalVideoFrame {
        let luma = self.plane(vk::Format::R8_UNORM, SIZE);
        let chroma = self.plane(vk::Format::R8G8_UNORM, SIZE / 2);
        let range = vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        };
        let barriers = [luma, chroma].map(|image| {
            vk::ImageMemoryBarrier2::default()
                .dst_stage_mask(vk::PipelineStageFlags2::CLEAR)
                .dst_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .image(image)
                .subresource_range(range)
        });
        unsafe {
            let cmd = self
                .device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(self.pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .expect("a command buffer")[0];
            self.device
                .begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
                .expect("recording starts");
            self.device.cmd_pipeline_barrier2(
                cmd,
                &vk::DependencyInfo::default().image_memory_barriers(&barriers),
            );
            for (image, value) in [
                (luma, [LUMA, 0.0, 0.0, 0.0]),
                (chroma, [CHROMA[0], CHROMA[1], 0.0, 0.0]),
            ] {
                self.device.cmd_clear_color_image(
                    cmd,
                    image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &vk::ClearColorValue { float32: value },
                    &[range],
                );
            }
            self.device.end_command_buffer(cmd).expect("recording ends");

            let semaphore = self.timeline();
            let commands = [vk::CommandBufferSubmitInfo::default().command_buffer(cmd)];
            let signals = [vk::SemaphoreSubmitInfo::default()
                .semaphore(semaphore)
                .value(DECODED)
                .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)];
            let submit = vk::SubmitInfo2::default()
                .command_buffer_infos(&commands)
                .signal_semaphore_infos(&signals);
            self.handles.queue_lock.lock();
            let submitted = self
                .device
                .queue_submit2(self.queue, &[submit], vk::Fence::null());
            self.handles.queue_lock.unlock();
            submitted.expect("the decode submits");
        }

        let semaphore = *self.semaphores.last().expect("just created");
        let state = Arc::new(Mutex::new(FrameState {
            value: DECODED,
            layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        }));
        let returned = Arc::new(AtomicBool::new(false));
        self.lent.push(Lent {
            semaphore,
            state: Arc::clone(&state),
            returned: Arc::clone(&returned),
        });
        ExternalVideoFrame {
            planes: vec![
                VideoPlane {
                    image: luma,
                    aspect: vk::ImageAspectFlags::COLOR,
                    view_format: vk::Format::R8_UNORM,
                },
                VideoPlane {
                    image: chroma,
                    aspect: vk::ImageAspectFlags::COLOR,
                    view_format: vk::Format::R8G8_UNORM,
                },
            ],
            chroma_packed: true,
            uv_scale: [1.0, 1.0],
            color_matrix: ycbcr_matrix(0.2126, 0.0722, 8, 1.0, true),
            lease: self.handles.lease(DecodedFrame {
                semaphore,
                images: [luma, chroma],
                state,
                returned,
            }),
        }
    }

    fn newest(&self) -> usize {
        self.lent.len() - 1
    }

    fn usable(&self, frame: usize, within: Duration) -> bool {
        let lent = &self.lent[frame];
        let value = lent.state.lock().expect("frame state").value;
        let semaphores = [lent.semaphore];
        let values = [value];
        let wait = vk::SemaphoreWaitInfo::default()
            .semaphores(&semaphores)
            .values(&values);
        unsafe {
            self.device
                .wait_semaphores(&wait, within.as_nanos() as u64)
                .is_ok()
        }
    }

    fn returned(&self, frame: usize, within: Duration) -> bool {
        let returned = &self.lent[frame].returned;
        let until = Instant::now() + within;
        while !returned.load(Ordering::Acquire) {
            if Instant::now() >= until {
                return false;
            }
            std::thread::sleep(RETURN_POLL);
        }
        self.usable(frame, within)
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        for frame in 0..self.lent.len() {
            let _ = self.usable(frame, PATIENCE);
        }
        unsafe {
            for plane in self.planes.drain(..) {
                self.device.destroy_image(plane.image, None);
                self.device.free_memory(plane.memory, None);
            }
            self.device.destroy_command_pool(self.pool, None);
            for semaphore in self.semaphores.drain(..) {
                self.device.destroy_semaphore(semaphore, None);
            }
        }
    }
}

fn expected_colour() -> [u8; 3] {
    let stored = |value: f32| (value * 255.0).round() / 255.0;
    let yuv = [stored(LUMA), stored(CHROMA[0]), stored(CHROMA[1])];
    ycbcr_matrix(0.2126, 0.0722, 8, 1.0, true).map(|row| {
        let value = row[0] * yuv[0] + row[1] * yuv[1] + row[2] * yuv[2] + row[3];
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    })
}

fn shows(
    renderer: &mut TestRenderer,
    scene: &mut Scene,
    target: &mut OffscreenTarget,
    texture: TextureId,
    colour: [u8; 3],
) -> bool {
    let centre = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
    (0..CONVERTER_COMPILE_POLLS).any(|_| {
        scene.notify_texture_updated(texture);
        renderer.draw(scene, &mut *target).expect("the scene draws");
        let pixels = renderer.read_pixels(target).expect("the target reads back");
        let shown = pixels[centre..centre + 3]
            .iter()
            .zip(colour)
            .all(|(got, want)| got.abs_diff(want) <= 3);
        if !shown {
            std::thread::sleep(CONVERTER_COMPILE_POLL);
        }
        shown
    })
}

fn setup() -> Option<(TestRenderer, Decoder, Scene, OffscreenTarget, TextureId)> {
    let mut renderer = rikka_test_renderer::video_renderer("rikka-video-frame-test")
        .expect("a Vulkan 1.3 device is required for these tests");
    let decoder = Decoder::new(&renderer)?;
    let texture = renderer
        .textures()
        .blank(SIZE, SIZE)
        .expect("a video texture");
    let target = renderer.create_target(SIZE, SIZE).expect("a target");
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        texture,
    ));
    Some((renderer, decoder, scene, target, texture))
}

#[test]
fn decoded_frame_converts_into_texture() {
    let Some((mut renderer, mut decoder, mut scene, mut target, texture)) = setup() else {
        return;
    };
    let frame = decoder.decode();
    let decoded = decoder.newest();
    unsafe {
        renderer
            .textures()
            .submit_video_frame(texture, frame)
            .expect("the frame is accepted")
    };
    assert!(
        shows(
            &mut renderer,
            &mut scene,
            &mut target,
            texture,
            expected_colour()
        ),
        "the texture never showed the decoded colour {:?}",
        expected_colour()
    );
    assert!(
        decoder.usable(decoded, PATIENCE),
        "the decoder never got its frame back"
    );
}

#[test]
fn frame_waiting_for_a_draw_owes_its_decoder_nothing() {
    let Some((mut renderer, mut decoder, mut scene, mut target, texture)) = setup() else {
        return;
    };
    let frame = decoder.decode();
    let decoded = decoder.newest();
    unsafe {
        renderer
            .textures()
            .submit_video_frame(texture, frame)
            .expect("the frame is accepted")
    };
    assert!(
        decoder.usable(decoded, PATIENCE),
        "a frame the renderer has not drawn yet holds its decoder back"
    );
    assert!(shows(
        &mut renderer,
        &mut scene,
        &mut target,
        texture,
        expected_colour()
    ));
    assert!(decoder.usable(decoded, PATIENCE));
}

#[test]
fn replaced_frame_returns_without_waiting_for_a_draw() {
    let Some((mut renderer, mut decoder, mut scene, mut target, texture)) = setup() else {
        return;
    };
    let first = decoder.decode();
    let replaced = decoder.newest();
    let second = decoder.decode();
    unsafe {
        let textures = renderer.textures();
        textures
            .submit_video_frame(texture, first)
            .expect("the first frame is accepted");
        textures
            .submit_video_frame(texture, second)
            .expect("the second frame replaces it");
    }
    assert!(
        decoder.returned(replaced, PATIENCE),
        "the replaced frame waited for a draw before it was handed back"
    );
    assert!(shows(
        &mut renderer,
        &mut scene,
        &mut target,
        texture,
        expected_colour()
    ));
}

#[test]
fn frame_dropped_before_the_renderer_takes_it_returns() {
    let Some((_renderer, mut decoder, _scene, _target, _texture)) = setup() else {
        return;
    };
    let frame = decoder.decode();
    let decoded = decoder.newest();
    drop(frame);
    assert!(
        decoder.returned(decoded, PATIENCE),
        "a dropped frame was never handed back to its decoder"
    );
}
