mod applications;
mod launch;
mod program;

pub use applications::{Applications, Handlers};
pub use launch::{launch, open_terminal, open_with_desktop};
pub use program::{is_program, run};
