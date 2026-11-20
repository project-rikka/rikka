pub fn serial_not_before(serial: u32, reference: u32) -> bool {
    serial.wrapping_sub(reference) < u32::MAX / 2
}
