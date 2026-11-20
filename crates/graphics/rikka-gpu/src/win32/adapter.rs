use windows::Win32::Foundation::LUID;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
    IDXGIAdapter1, IDXGIFactory6,
};

pub(crate) fn preferred_adapter() -> Option<[u8; 8]> {
    let factory = match unsafe { CreateDXGIFactory1::<IDXGIFactory6>() } {
        Ok(factory) => factory,
        Err(error) => {
            tracing::debug!("failed to ask DXGI for the high-performance adapter: {error}");
            return None;
        }
    };
    (0..)
        .map_while(|index| unsafe {
            factory
                .EnumAdapterByGpuPreference::<IDXGIAdapter1>(
                    index,
                    DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
                )
                .ok()
        })
        .filter_map(|adapter| unsafe { adapter.GetDesc1() }.ok())
        .find(|desc| desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 == 0)
        .map(|desc| luid_bytes(desc.AdapterLuid))
}

fn luid_bytes(luid: LUID) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&luid.LowPart.to_le_bytes());
    bytes[4..].copy_from_slice(&luid.HighPart.to_le_bytes());
    bytes
}
