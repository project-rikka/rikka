use crate::error::Error;

pub(crate) fn failed(name: &'static str) -> impl Fn(rikka_worker::Error<Error>) -> Error {
    move |error| match error {
        rikka_worker::Error::Spawn(source) => Error::NoThread { name, source },
        rikka_worker::Error::Startup(error) => error,
        rikka_worker::Error::Exited => Error::ThreadDied { name },
    }
}
