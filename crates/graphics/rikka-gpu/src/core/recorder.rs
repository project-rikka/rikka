use ash::vk;

use crate::core::device::RenderDevice;
use crate::error::Result;

pub(crate) struct Recorder {
    pool: vk::CommandPool,
    idle: Vec<vk::CommandBuffer>,
    busy: Vec<(u64, vk::CommandBuffer)>,
}

impl Recorder {
    pub(crate) fn new(device: &RenderDevice, family: u32) -> Result<Recorder> {
        let info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(family);
        let pool = unsafe { device.handle().create_command_pool(&info, None)? };
        Ok(Recorder {
            pool,
            idle: Vec::new(),
            busy: Vec::new(),
        })
    }

    pub(crate) fn command_buffer(
        &mut self,
        device: &RenderDevice,
        finished: u64,
    ) -> Result<vk::CommandBuffer> {
        self.settle(finished);
        if let Some(cmd) = self.idle.pop() {
            unsafe {
                device
                    .handle()
                    .reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?
            };
            return Ok(cmd);
        }
        let info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        Ok(unsafe { device.handle().allocate_command_buffers(&info)? }[0])
    }

    pub(crate) fn submitted(&mut self, value: u64, cmd: vk::CommandBuffer) {
        self.busy.push((value, cmd));
    }

    pub(crate) fn recycle(&mut self, cmd: vk::CommandBuffer) {
        self.idle.push(cmd);
    }

    pub(crate) fn trim(&mut self, device: &RenderDevice, finished: u64) {
        self.settle(finished);
        if self.idle.is_empty() {
            return;
        }
        unsafe {
            device.handle().free_command_buffers(self.pool, &self.idle);
            device
                .handle()
                .trim_command_pool(self.pool, vk::CommandPoolTrimFlags::empty());
        }
        self.idle.clear();
    }

    fn settle(&mut self, finished: u64) {
        let mut at = 0;
        while at < self.busy.len() {
            match self.busy[at].0 <= finished {
                true => {
                    let (_, cmd) = self.busy.swap_remove(at);
                    self.idle.push(cmd);
                }
                false => at += 1,
            }
        }
    }

    pub(crate) fn destroy(self, device: &RenderDevice) {
        unsafe { device.handle().destroy_command_pool(self.pool, None) };
    }
}
