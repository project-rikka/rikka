pub(crate) type PictureSink = Box<dyn Fn(u32, u32, Vec<u8>) + Send>;
