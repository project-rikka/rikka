#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct KeyFlags {
    pub certify: bool,
    pub sign: bool,
    pub encrypt_communications: bool,
    pub encrypt_storage: bool,
    pub authenticate: bool,
}

impl KeyFlags {
    #[must_use]
    pub fn can_encrypt(&self) -> bool {
        self.encrypt_communications || self.encrypt_storage
    }
}

impl From<proton_rpgp::pgp::packet::KeyFlags> for KeyFlags {
    fn from(flags: proton_rpgp::pgp::packet::KeyFlags) -> Self {
        KeyFlags {
            certify: flags.certify(),
            sign: flags.sign(),
            encrypt_communications: flags.encrypt_comms(),
            encrypt_storage: flags.encrypt_storage(),
            authenticate: flags.authentication(),
        }
    }
}
