use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use rikka_seam::{Call, EventInfo, Manifest, Method, MethodInfo, Service, ServiceContext};
use rikka_tool::{Function, Reply};
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tokio::sync::oneshot;

use crate::error::Error;
use crate::protocol::{
    CALL, CANCEL, CAPABILITY, CHANGED, Calling, Cancelling, DESCRIBE, Declared, Described, Outcome,
    STOPPED, VERSION, WINDOW, WINDOWS,
};
use crate::serving::Shared;

const WINDOW_SAID: &str =
    "The window to work, named as the Windows tool lists it; the window used last when omitted.";

const WINDOWS_SAID: &str = "Lists this app's open windows by name, the one used last first. Every other tool works that window unless given another `window`.";

type Running = Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>;

pub(crate) struct AssistService {
    shared: Arc<Shared>,
    working: Handle,
    running: Running,
}

impl AssistService {
    pub(crate) fn new(shared: Arc<Shared>, working: Handle) -> AssistService {
        AssistService {
            shared,
            working,
            running: Running::default(),
        }
    }

    fn describe(&self) -> Described {
        let offering = self.shared.offering();
        let seats = self.shared.seats();
        let tools = seats.first().map_or_else(Vec::new, |seat| {
            seat.functions()
                .iter()
                .map(|function| Declared {
                    name: function.name().to_owned(),
                    description: function.tool().description().to_owned(),
                    schema: windowed(function.tool().schema()),
                    gist: offering.gist_of(function.name()),
                })
                .chain(std::iter::once(windows()))
                .collect()
        });
        Described {
            kit: offering.kit().to_owned(),
            title: offering.title().to_owned(),
            about: offering.about().to_owned(),
            instructions: offering.instructions().to_owned(),
            tools,
        }
    }

    fn call(&self, call: Call) {
        let calling: Calling = match call.decode() {
            Ok(calling) => calling,
            Err(error) => return call.fail(error),
        };
        let Calling {
            call: id,
            tool,
            mut input,
        } = calling;
        let window = taken_window(&mut input);

        let function = {
            let seats = self.shared.seats();
            if tool == WINDOWS {
                return call.encode(&Outcome::from_reply(Ok(Reply::new(seats.listing()))));
            }
            match seats
                .select(window.as_deref())
                .and_then(|seat| seat.function(&tool))
            {
                Ok(function) => function,
                Err(reason) => return call.encode(&Outcome::failed(reason)),
            }
        };

        let (_, responder) = call.defer();
        let (stop, stopped) = oneshot::channel::<()>();
        let running = Arc::clone(&self.running);
        self.running.lock().insert(id.clone(), stop);
        self.working.spawn(async move {
            let outcome = tokio::select! {
                answered = function.call(input) => Outcome::from_reply(answered),
                _ = stopped => Outcome::failed(STOPPED),
            };
            running.lock().remove(&id);
            responder.encode(&outcome);
        });
    }

    fn cancel(&self, call: Call) {
        match call.decode::<Cancelling>() {
            Ok(cancelling) => {
                if let Some(stop) = self.running.lock().remove(&cancelling.call) {
                    let _ = stop.send(());
                }
                call.done();
            }
            Err(error) => call.fail(error),
        }
    }
}

impl Service for AssistService {
    fn manifest(&self) -> Manifest {
        Manifest::new(CAPABILITY, VERSION)
            .summary("An app's own tools, offered to the desktop's assistants")
            .method(MethodInfo::new(DESCRIBE).summary("The app, its instructions and its tools"))
            .method(MethodInfo::new(CALL).summary("Runs one of the app's tools"))
            .method(MethodInfo::new(CANCEL).summary("Stops a tool that is still running"))
            .event(
                EventInfo::new(CHANGED)
                    .summary("The app's windows or tools changed")
                    .latest(),
            )
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.shared.publish_with(seam.publisher().clone());
    }

    fn call(&mut self, _: &mut (), method: Method, call: Call) {
        match &*method {
            DESCRIBE => call.encode(&self.describe()),
            CALL => AssistService::call(self, call),
            CANCEL => self.cancel(call),
            _ => call.unknown_method(),
        }
    }
}

fn windowed(schema: &Value) -> Value {
    let mut schema = schema.clone();
    if let Some(declared) = schema.as_object_mut() {
        let properties = declared.entry("properties").or_insert_with(|| json!({}));
        if let Some(properties) = properties.as_object_mut() {
            properties.insert(
                WINDOW.to_owned(),
                json!({ "type": "string", "description": WINDOW_SAID }),
            );
        }
    }
    schema
}

pub(crate) fn unwindowed(function: &Function) -> Result<(), Error> {
    let refused = |reason| Error::Unofferable {
        tool: function.name().to_owned(),
        reason,
    };
    if function.name() == WINDOWS {
        return Err(refused("the name is kept for listing windows"));
    }
    if function.tool().schema()["properties"].get(WINDOW).is_some() {
        return Err(refused("the `window` input is kept for choosing a window"));
    }
    Ok(())
}

fn windows() -> Declared {
    Declared {
        name: WINDOWS.to_owned(),
        description: WINDOWS_SAID.to_owned(),
        schema: json!({ "type": "object", "properties": {} }),
        gist: Vec::new(),
    }
}

fn taken_window(input: &mut Value) -> Option<String> {
    match input.as_object_mut()?.remove(WINDOW)? {
        Value::String(name) => Some(name),
        _ => None,
    }
}
