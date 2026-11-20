use std::sync::Arc;
use std::time::Duration;

use rikka_audio_decode::{Decoder, DsdReader, Result};
use rikka_worker::Worker;

use crate::source::feed::{Close, Closing, Feed, Idle, Stopped};

pub(crate) const NAME: &str = "rikka-hifi-decode";

pub(crate) trait Producing<T>: Send + 'static {
    fn produce(&mut self, into: &mut Vec<T>) -> Result<bool>;
    fn seek(&mut self, to: Duration) -> Result<Duration>;
}

impl Producing<f32> for Decoder {
    fn produce(&mut self, into: &mut Vec<f32>) -> Result<bool> {
        self.read(into)
    }

    fn seek(&mut self, to: Duration) -> Result<Duration> {
        Decoder::seek(self, to)
    }
}

impl Producing<u8> for DsdReader {
    fn produce(&mut self, into: &mut Vec<u8>) -> Result<bool> {
        self.read(into)
    }

    fn seek(&mut self, to: Duration) -> Result<Duration> {
        DsdReader::seek(self, to)
    }
}

pub(crate) fn spawn<T, P>(
    producer: P,
    feed: &Arc<Feed<T>>,
) -> Result<Worker<Closing>, rikka_worker::Error<std::convert::Infallible>>
where
    T: Copy + Send + 'static,
    P: Producing<T>,
{
    let fed = Arc::clone(feed);
    Worker::spawn(
        NAME,
        Closing(Arc::clone(feed) as Arc<dyn Close>),
        move || Ok(move || run(producer, &fed)),
    )
}

fn run<T: Copy, P: Producing<T>>(mut producer: P, feed: &Feed<T>) {
    let mut block = Vec::new();
    loop {
        if let Some(to) = feed.take_seek_request() {
            match producer.seek(to) {
                Ok(reached) => feed.restart(reached),
                Err(error) => feed.fail(error),
            }
            continue;
        }
        block.clear();
        match producer.produce(&mut block) {
            Ok(true) => match feed.push(&block) {
                Ok(()) | Err(Stopped::Seeking) => {}
                Err(Stopped::Closed) => return,
            },
            Ok(false) => {
                feed.finish();
                if feed.idle() == Idle::Closed {
                    return;
                }
            }
            Err(error) => {
                feed.fail(error);
                if feed.idle() == Idle::Closed {
                    return;
                }
            }
        }
    }
}
