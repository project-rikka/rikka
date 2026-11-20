use std::ptr;
use std::sync::Arc;

use ash::vk;

use crate::capabilities::Capabilities;
use crate::device::VideoDevice;
use crate::error::Result;
use crate::profile::Profile;
use crate::syntax::Syntax;

pub(crate) struct Session {
    device: Arc<VideoDevice>,
    pub(crate) session: vk::VideoSessionKHR,
    memory: Vec<vk::DeviceMemory>,
    pub(crate) parameters: vk::VideoSessionParametersKHR,
    pub(crate) header: Vec<u8>,
}

pub(crate) struct SessionRequest<'a> {
    pub(crate) profile: &'a Profile,
    pub(crate) capabilities: &'a Capabilities,
    pub(crate) syntax: &'a Syntax,
    pub(crate) coded: vk::Extent2D,
    pub(crate) slots: u32,
    pub(crate) quality: u32,
}

impl Session {
    pub(crate) fn new(device: &Arc<VideoDevice>, request: &SessionRequest<'_>) -> Result<Session> {
        let mut info = vk::VideoSessionCreateInfoKHR::default()
            .queue_family_index(device.encode_family)
            .picture_format(request.capabilities.picture_format)
            .max_coded_extent(request.coded)
            .reference_picture_format(request.capabilities.reference_format)
            .max_dpb_slots(request.slots)
            .max_active_reference_pictures(1)
            .std_header_version(&request.capabilities.header_version);
        info.p_video_profile = request.profile.info();
        let mut session = vk::VideoSessionKHR::null();
        unsafe {
            (device.queue_fns.create_video_session_khr)(
                device.device.handle(),
                &info,
                ptr::null(),
                &mut session,
            )
        }
        .result()?;
        let mut made = Session {
            device: Arc::clone(device),
            session,
            memory: Vec::new(),
            parameters: vk::VideoSessionParametersKHR::null(),
            header: Vec::new(),
        };
        made.bind_memory()?;
        made.create_parameters(request)?;
        made.header = made.read_header(request.syntax)?;
        Ok(made)
    }

    fn bind_memory(&mut self) -> Result<()> {
        let device = Arc::clone(&self.device);
        let query = device.queue_fns.get_video_session_memory_requirements_khr;
        let handle = device.device.handle();
        let mut count = 0;
        unsafe { (query)(handle, self.session, &mut count, ptr::null_mut()) }.result()?;
        let mut requirements =
            vec![vk::VideoSessionMemoryRequirementsKHR::default(); count as usize];
        unsafe { (query)(handle, self.session, &mut count, requirements.as_mut_ptr()) }.result()?;
        let mut binds = Vec::with_capacity(requirements.len());
        for requirement in &requirements {
            let memory = device.allocate(
                requirement.memory_requirements,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                vk::MemoryPropertyFlags::empty(),
            )?;
            self.memory.push(memory);
            binds.push(
                vk::BindVideoSessionMemoryInfoKHR::default()
                    .memory_bind_index(requirement.memory_bind_index)
                    .memory(memory)
                    .memory_offset(0)
                    .memory_size(requirement.memory_requirements.size),
            );
        }
        unsafe {
            (device.queue_fns.bind_video_session_memory_khr)(
                handle,
                self.session,
                binds.len() as u32,
                binds.as_ptr(),
            )
        }
        .result()?;
        Ok(())
    }

    fn create_parameters(&mut self, request: &SessionRequest<'_>) -> Result<()> {
        let codec = request.syntax.parameters();
        let mut quality =
            vk::VideoEncodeQualityLevelInfoKHR::default().quality_level(request.quality);
        quality.p_next = codec.next();
        let mut info =
            vk::VideoSessionParametersCreateInfoKHR::default().video_session(self.session);
        info.p_next = ptr::from_ref(&quality).cast();
        unsafe {
            (self.device.queue_fns.create_video_session_parameters_khr)(
                self.device.device.handle(),
                &info,
                ptr::null(),
                &mut self.parameters,
            )
        }
        .result()?;
        Ok(())
    }

    fn read_header(&self, syntax: &Syntax) -> Result<Vec<u8>> {
        let codec = syntax.header_query();
        let mut info = vk::VideoEncodeSessionParametersGetInfoKHR::default()
            .video_session_parameters(self.parameters);
        info.p_next = codec.next();
        let mut feedback = vk::VideoEncodeSessionParametersFeedbackInfoKHR::default();
        let read = self
            .device
            .encode_fns
            .get_encoded_video_session_parameters_khr;
        let handle = self.device.device.handle();
        let mut size = 0;
        unsafe { (read)(handle, &info, &mut feedback, &mut size, ptr::null_mut()) }.result()?;
        let mut header = vec![0_u8; size];
        unsafe {
            (read)(
                handle,
                &info,
                &mut feedback,
                &mut size,
                header.as_mut_ptr().cast(),
            )
        }
        .result()?;
        header.truncate(size);
        if feedback.has_overrides == vk::TRUE {
            tracing::debug!("the driver adjusted the parameter sets");
        }
        Ok(header)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let handle = self.device.device.handle();
        unsafe {
            if self.parameters != vk::VideoSessionParametersKHR::null() {
                (self.device.queue_fns.destroy_video_session_parameters_khr)(
                    handle,
                    self.parameters,
                    ptr::null(),
                );
            }
            (self.device.queue_fns.destroy_video_session_khr)(handle, self.session, ptr::null());
            for memory in self.memory.drain(..) {
                self.device.device.free_memory(memory, None);
            }
        }
    }
}
