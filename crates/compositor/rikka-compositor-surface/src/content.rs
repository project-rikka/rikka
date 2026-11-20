use crate::dmabuf::Dmabuf;
use crate::shm::ShmBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u32,
    pub g: u32,
    pub b: u32,
    pub a: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum BufferContent<'a> {
    Shm(ShmBuffer<'a>),
    SinglePixel(Rgba),
    Dmabuf(&'a Dmabuf),
}
