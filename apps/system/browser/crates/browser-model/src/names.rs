#[derive(Clone, Copy, Debug)]
pub struct Names {
    pub profile: fn() -> String,
    pub numbered_profile: fn(usize) -> String,
    pub space: fn() -> String,
    pub numbered_space: fn(usize) -> String,
    pub folder: fn() -> String,
}
