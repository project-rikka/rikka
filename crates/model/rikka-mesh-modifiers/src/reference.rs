use rikka_file_format::{Codec, DecodeError, Reader, Writer};

macro_rules! reference {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn new(id: u64) -> $name {
                $name(id)
            }

            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl Codec for $name {
            type Error = DecodeError;

            fn encode(&self, out: &mut Writer<'_>) {
                out.put(&self.0);
            }

            fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
                input.get().map($name)
            }
        }
    };
}

reference!(ObjectRef);
reference!(TextureRef);
reference!(CollectionRef);
reference!(MaterialRef);
