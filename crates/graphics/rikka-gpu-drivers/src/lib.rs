mod kernel;
mod manifest;
mod shipped;
mod system;

pub use shipped::Drivers;
pub use system::{VULKAN_SELECT_ENV, narrow_system};

fn extend_unique<T: PartialEq>(into: &mut Vec<T>, items: impl IntoIterator<Item = T>) {
    for item in items {
        if !into.contains(&item) {
            into.push(item);
        }
    }
}
