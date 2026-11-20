use futures_util::Stream;
use zbus::blocking::Connection;

use crate::changes;
use crate::controller::{ControllerProxyBlocking, Entry};
use crate::engine::Engine;
use crate::error::{Error, Result};
use crate::group::Group;
use crate::member::Member;

pub struct Fcitx {
    controller: ControllerProxyBlocking<'static>,
}

impl Fcitx {
    pub fn connect() -> Result<Fcitx> {
        let connection = Connection::session().map_err(Error::bus)?;
        let controller = ControllerProxyBlocking::new(&connection).map_err(Error::bus)?;
        Ok(Fcitx { controller })
    }

    pub fn changes() -> Result<impl Stream<Item = ()> + Send + 'static> {
        changes::start()
    }

    pub fn engines(&self) -> Result<Vec<Engine>> {
        let available = self
            .controller
            .available_input_methods()
            .map_err(|error| call_error("AvailableInputMethods", error))?;
        Ok(available
            .into_iter()
            .map(
                |(id, name, native_name, icon, label, language, configurable)| Engine {
                    id,
                    name,
                    native_name,
                    icon,
                    label,
                    language,
                    configurable,
                },
            )
            .collect())
    }

    pub fn group(&self) -> Result<Group> {
        let name = self
            .controller
            .current_input_method_group()
            .map_err(|error| call_error("CurrentInputMethodGroup", error))?;
        let (default_layout, entries) = self
            .controller
            .input_method_group_info(&name)
            .map_err(|error| call_error("InputMethodGroupInfo", error))?;
        Ok(Group {
            name,
            default_layout,
            members: entries
                .into_iter()
                .map(|(engine, layout)| Member { engine, layout })
                .collect(),
        })
    }

    pub fn set_group(&self, group: &Group) -> Result<()> {
        let entries: Vec<Entry> = group
            .members
            .iter()
            .map(|member| (member.engine.clone(), member.layout.clone()))
            .collect();
        self.controller
            .set_input_method_group_info(&group.name, &group.default_layout, &entries)
            .map_err(|error| call_error("SetInputMethodGroupInfo", error))
    }

    pub fn configure(&self, engine: &str) -> Result<()> {
        self.controller
            .configure_input_method(engine)
            .map_err(|error| call_error("ConfigureIM", error))
    }
}

impl std::fmt::Debug for Fcitx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fcitx").finish_non_exhaustive()
    }
}

fn call_error(call: &'static str, error: zbus::Error) -> Error {
    match &error {
        zbus::Error::MethodError(name, _, _) if name.as_str() == UNKNOWN_SERVICE => {
            Error::NotRunning
        }
        zbus::Error::Variant(reason) => Error::Unintelligible {
            call,
            reason: reason.to_string(),
        },
        _ => Error::Refused {
            call,
            reason: error.to_string(),
        },
    }
}

const UNKNOWN_SERVICE: &str = "org.freedesktop.DBus.Error.ServiceUnknown";
