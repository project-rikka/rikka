#[cfg(unix)]
mod session;
#[cfg(windows)]
mod system;

#[cfg(unix)]
pub use session::{Appearance, ResolvedAppearance};
#[cfg(windows)]
pub use system::{Appearance, is_appearance_change};
