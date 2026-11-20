use rikka_coded::Coded;

const OVERHEAD: usize = std::mem::size_of::<Coded>() + 2 * std::mem::size_of::<usize>();

pub(crate) fn byte_size(packet: &Coded) -> usize {
    packet.data.len() + OVERHEAD
}
