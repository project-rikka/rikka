use x11rb_protocol::x11_utils::{ExtInfoProvider, ExtensionInformation};

#[derive(Debug, Default)]
pub(crate) struct Extensions {
    known: Vec<(&'static str, ExtensionInformation)>,
}

impl Extensions {
    pub(crate) fn insert(&mut self, name: &'static str, information: ExtensionInformation) {
        self.known.push((name, information));
    }

    pub(crate) fn opcode(&self, name: &str) -> Option<u8> {
        self.known
            .iter()
            .find(|(known, _)| *known == name)
            .map(|(_, information)| information.major_opcode)
    }

    fn latest_at_most(
        &self,
        code: u8,
        first: impl Fn(&ExtensionInformation) -> u8,
    ) -> Option<(&str, ExtensionInformation)> {
        self.known
            .iter()
            .filter(|(_, information)| first(information) != 0 && first(information) <= code)
            .max_by_key(|(_, information)| first(information))
            .map(|(name, information)| (*name, *information))
    }
}

impl ExtInfoProvider for Extensions {
    fn get_from_major_opcode(&self, major_opcode: u8) -> Option<(&str, ExtensionInformation)> {
        self.known
            .iter()
            .find(|(_, information)| information.major_opcode == major_opcode)
            .map(|(name, information)| (*name, *information))
    }

    fn get_from_event_code(&self, event_code: u8) -> Option<(&str, ExtensionInformation)> {
        self.latest_at_most(event_code, |information| information.first_event)
    }

    fn get_from_error_code(&self, error_code: u8) -> Option<(&str, ExtensionInformation)> {
        self.latest_at_most(error_code, |information| information.first_error)
    }
}
