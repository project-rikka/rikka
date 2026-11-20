pub trait Id: Copy + Eq + Ord + std::hash::Hash + std::fmt::Debug {
    fn from_parts(index: u32, generation: u32) -> Self;

    fn index(self) -> u32;

    fn generation(self) -> u32;
}

#[macro_export]
macro_rules! id {
    ($(#[$meta:meta])* $visibility:vis $name:ident) => {
        $(#[$meta])*
        #[repr(C)]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        $visibility struct $name {
            pub index: u32,
            pub generation: u32,
        }

        impl $crate::Id for $name {
            #[inline]
            fn from_parts(index: u32, generation: u32) -> Self {
                Self { index, generation }
            }

            #[inline]
            fn index(self) -> u32 {
                self.index
            }

            #[inline]
            fn generation(self) -> u32 {
                self.generation
            }
        }

        unsafe impl $crate::__bytemuck::Zeroable for $name {}
        unsafe impl $crate::__bytemuck::Pod for $name {}

        impl $crate::__file_format::Codec for $name {
            type Error = $crate::__file_format::DecodeError;

            fn encode(&self, out: &mut $crate::__file_format::Writer<'_>) {
                out.put(&self.index);
                out.put(&self.generation);
            }

            fn decode(
                input: &mut $crate::__file_format::Reader<'_>,
            ) -> ::std::result::Result<Self, $crate::__file_format::DecodeError> {
                Ok(Self {
                    index: input.get()?,
                    generation: input.get()?,
                })
            }
        }
    };
}
