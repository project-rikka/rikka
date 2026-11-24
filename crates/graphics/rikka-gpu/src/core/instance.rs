use std::ffi::{CStr, CString, c_char, c_void};

use ash::{ext, vk};

#[cfg(unix)]
use crate::core::direct_drivers::DirectDrivers;
use crate::error::{Error, Result};

const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";

const VALIDATION_CHECKS: [&CStr; 4] = [
    c"validate_sync",
    c"validate_best_practices",
    c"validate_best_practices_amd",
    c"validate_best_practices_nvidia",
];

const DIRECT_DRIVER_LOADING_SINCE: u32 = vk::make_api_version(0, 1, 3, 236);

static ENABLED: vk::Bool32 = vk::TRUE;

fn validation_settings() -> Vec<vk::LayerSettingEXT<'static>> {
    VALIDATION_CHECKS
        .iter()
        .map(|check| {
            let mut setting = vk::LayerSettingEXT::default()
                .layer_name(VALIDATION_LAYER)
                .setting_name(check)
                .ty(vk::LayerSettingTypeEXT::BOOL32);
            setting.value_count = 1;
            setting.p_values = std::ptr::from_ref(&ENABLED).cast();
            setting
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ownership {
    Owned,
    Borrowed,
}

pub struct VulkanInstance {
    entry: ash::Entry,
    instance: ash::Instance,
    debug: Option<DebugMessenger>,
    extensions: Vec<CString>,
    #[cfg(unix)]
    _drivers: Option<DirectDrivers>,
    ownership: Ownership,
}

struct DebugMessenger {
    loader: ext::debug_utils::Instance,
    messenger: vk::DebugUtilsMessengerEXT,
}

impl VulkanInstance {
    pub fn new(app_name: &str, validation: bool) -> Result<Self> {
        let entry = unsafe { ash::Entry::load()? };

        let instance_version =
            unsafe { entry.try_enumerate_instance_version()? }.unwrap_or(vk::API_VERSION_1_0);
        if instance_version < vk::API_VERSION_1_3 {
            return Err(Error::Unsupported(format!(
                "Vulkan 1.3 instance required, driver reports {}.{}",
                vk::api_version_major(instance_version),
                vk::api_version_minor(instance_version),
            )));
        }

        let validation_enabled = validation && Self::validation_layer_available(&entry)?;
        if validation && !validation_enabled {
            tracing::warn!("VK_LAYER_KHRONOS_validation requested but not installed");
        }
        let layer_settings = validation_enabled && Self::layer_settings_available(&entry)?;
        if validation_enabled && !layer_settings {
            tracing::warn!(
                "validation layer lacks VK_EXT_layer_settings; synchronization and \
                 best-practices validation disabled"
            );
        }

        #[cfg(unix)]
        let drivers = DirectDrivers::load()?;
        #[cfg(unix)]
        let direct = drivers.is_some();
        #[cfg(windows)]
        let direct = false;

        let mut extensions: Vec<*const c_char> = Vec::new();
        let available = match direct {
            true => Self::loader_provided(instance_version)?,
            false => Self::available(&entry)?,
        };
        let offers = |name: &CStr| available.iter().any(|offered| offered.as_c_str() == name);
        let debug_utils = offers(ext::debug_utils::NAME);
        if debug_utils {
            extensions.push(ext::debug_utils::NAME.as_ptr());
        }
        if layer_settings {
            extensions.push(ext::layer_settings::NAME.as_ptr());
        }
        #[cfg(windows)]
        if crate::win32::SURFACE_EXTENSIONS
            .iter()
            .all(|name| offers(name))
        {
            extensions.extend(
                crate::win32::SURFACE_EXTENSIONS
                    .iter()
                    .map(|name| name.as_ptr()),
            );
        }
        if direct {
            extensions.push(vk::LUNARG_DIRECT_DRIVER_LOADING_NAME.as_ptr());
        }

        let mut layers: Vec<*const c_char> = Vec::new();
        if validation_enabled {
            layers.push(VALIDATION_LAYER.as_ptr());
        }

        let settings = validation_settings();
        let mut settings_info = vk::LayerSettingsCreateInfoEXT::default().settings(&settings);
        let app_name = CString::new(app_name).unwrap_or_else(|_| c"rikka".to_owned());
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .engine_name(c"rikka-renderer")
            .api_version(vk::API_VERSION_1_3);
        let mut create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_layer_names(&layers)
            .enabled_extension_names(&extensions);
        if layer_settings {
            create_info = create_info.push_next(&mut settings_info);
        }
        #[cfg(unix)]
        let mut driver_list = drivers.as_ref().map(|drivers| {
            vk::DirectDriverLoadingListLUNARG::default()
                .mode(vk::DirectDriverLoadingModeLUNARG::EXCLUSIVE)
                .drivers(drivers.infos())
        });
        #[cfg(unix)]
        if let Some(driver_list) = driver_list.as_mut() {
            create_info = create_info.push_next(driver_list);
        }

        let instance = unsafe { entry.create_instance(&create_info, None)? };

        let extensions = extensions
            .iter()
            .map(|&ptr| unsafe { CStr::from_ptr(ptr) }.to_owned())
            .collect();

        let debug = if validation_enabled && debug_utils {
            match Self::create_debug_messenger(&entry, &instance) {
                Ok(debug) => Some(debug),
                Err(err) => {
                    tracing::warn!("failed to create debug messenger: {err}");
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            entry,
            instance,
            debug,
            extensions,
            #[cfg(unix)]
            _drivers: drivers,
            ownership: Ownership::Owned,
        })
    }

    pub(crate) unsafe fn adopt(
        get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
        instance: vk::Instance,
        api_version: u32,
        extensions: Vec<CString>,
    ) -> Result<Self> {
        if api_version < vk::API_VERSION_1_3 {
            return Err(Error::Unsupported(format!(
                "the host instance targets Vulkan {}.{}, 1.3 is required",
                vk::api_version_major(api_version),
                vk::api_version_minor(api_version),
            )));
        }
        let entry = unsafe {
            ash::Entry::from_static_fn(ash::StaticFn {
                get_instance_proc_addr,
            })
        };
        let instance = unsafe { ash::Instance::load(entry.static_fn(), instance) };
        Ok(Self {
            entry,
            instance,
            debug: None,
            extensions,
            #[cfg(unix)]
            _drivers: None,
            ownership: Ownership::Borrowed,
        })
    }

    pub(crate) fn extensions(&self) -> &[CString] {
        &self.extensions
    }

    #[cfg(windows)]
    pub(crate) fn has_extension(&self, name: &CStr) -> bool {
        self.extensions
            .iter()
            .any(|enabled| enabled.as_c_str() == name)
    }

    fn available(entry: &ash::Entry) -> Result<Vec<CString>> {
        let extensions = unsafe { entry.enumerate_instance_extension_properties(None)? };
        Ok(extensions
            .iter()
            .filter_map(|extension| extension.extension_name_as_c_str().ok())
            .map(CStr::to_owned)
            .collect())
    }

    fn loader_provided(loader_version: u32) -> Result<Vec<CString>> {
        if loader_version < DIRECT_DRIVER_LOADING_SINCE {
            return Err(Error::Unsupported(format!(
                "Vulkan loader {}.{}.{} lacks VK_LUNARG_direct_driver_loading",
                vk::api_version_major(loader_version),
                vk::api_version_minor(loader_version),
                vk::api_version_patch(loader_version),
            )));
        }
        Ok([
            ext::debug_utils::NAME,
            vk::LUNARG_DIRECT_DRIVER_LOADING_NAME,
        ]
        .map(CStr::to_owned)
        .into())
    }

    fn validation_layer_available(entry: &ash::Entry) -> Result<bool> {
        let layers = unsafe { entry.enumerate_instance_layer_properties()? };
        Ok(layers.iter().any(|layer| {
            layer
                .layer_name_as_c_str()
                .is_ok_and(|name| name == VALIDATION_LAYER)
        }))
    }
    fn layer_settings_available(entry: &ash::Entry) -> Result<bool> {
        let extensions =
            unsafe { entry.enumerate_instance_extension_properties(Some(VALIDATION_LAYER))? };
        Ok(extensions.iter().any(|extension| {
            extension
                .extension_name_as_c_str()
                .is_ok_and(|name| name == ext::layer_settings::NAME)
        }))
    }

    fn create_debug_messenger(
        entry: &ash::Entry,
        instance: &ash::Instance,
    ) -> Result<DebugMessenger> {
        let loader = ext::debug_utils::Instance::new(entry, instance);
        let create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                    | vk::DebugUtilsMessageTypeFlagsEXT::GENERAL,
            )
            .pfn_user_callback(Some(debug_callback));
        let messenger = unsafe { loader.create_debug_utils_messenger(&create_info, None)? };
        Ok(DebugMessenger { loader, messenger })
    }

    pub fn entry(&self) -> &ash::Entry {
        &self.entry
    }

    pub fn handle(&self) -> &ash::Instance {
        &self.instance
    }
}

impl Drop for VulkanInstance {
    fn drop(&mut self) {
        if self.ownership == Ownership::Borrowed {
            return;
        }
        unsafe {
            if let Some(debug) = self.debug.take() {
                debug
                    .loader
                    .destroy_debug_utils_messenger(debug.messenger, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}

unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _types: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut c_void,
) -> vk::Bool32 {
    let message = unsafe {
        (*data)
            .message_as_c_str()
            .map(CStr::to_string_lossy)
            .unwrap_or_default()
    };
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        tracing::error!(target: "rikka_renderer::vulkan", "{message}");
    } else {
        tracing::warn!(target: "rikka_renderer::vulkan", "{message}");
    }
    vk::FALSE
}

impl std::fmt::Debug for VulkanInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanInstance").finish_non_exhaustive()
    }
}
