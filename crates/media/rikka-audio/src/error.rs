#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no audio output device available")]
    NoOutputDevice,

    #[error("audio output device \"{0}\" not found")]
    DeviceNotFound(String),

    #[error("an audio {0} needs at least one channel")]
    NoChannels(Role),

    #[error("{channels} audio {role} channels exceed the supported maximum of {most}")]
    TooManyChannels {
        role: Role,
        channels: u16,
        most: usize,
    },

    #[error("no audio input device available")]
    NoInputDevice,

    #[error("sound server offers no monitor source for system audio")]
    NoSystemAudio,

    #[error("thread `{name}` died on startup")]
    ThreadDied { name: &'static str },

    #[cfg(unix)]
    #[error("PipeWire: {0}")]
    PipeWire(#[from] pipewire::Error),

    #[cfg(unix)]
    #[error("the sound server did not answer")]
    Unanswered,

    #[cfg(windows)]
    #[error("WASAPI: {0}")]
    Wasapi(#[from] windows::core::Error),

    #[error(transparent)]
    Opus(#[from] rikka_opus::Error),

    #[error("Opus cannot encode {0} Hz")]
    OpusRate(u32),

    #[error("failed to spawn thread `{name}`")]
    NoThread {
        name: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Decode(#[from] rikka_audio_decode::Error),

    #[error("{samples} samples do not make whole frames of {channels} channels")]
    PartialFrame { samples: usize, channels: usize },

    #[error("engine command queue is full")]
    CommandQueueFull,

    #[error("feed voice stopped")]
    FeedClosed,

    #[error("audio output stalled")]
    FeedStalled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Output,
    Input,
    Feed,
    Source,
}

impl std::fmt::Display for Role {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Role::Output => "output",
            Role::Input => "input",
            Role::Feed => "feed",
            Role::Source => "source",
        })
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
