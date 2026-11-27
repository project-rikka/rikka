use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use rikka_picture::Feed;

use crate::delivery::{Delivery, Picture, Preview, Ticket};
use crate::error::BoothError;
use crate::loader::Loader;
use crate::pool::{Pool, Queue};
use crate::sitter::Order;
use crate::studio;
use crate::work::{self, Job, Message, Task};

const LOADERS: usize = 2;
const LOADER_THREAD: &str = "rikka-animate-booth-load";
const STUDIO_THREAD: &str = "rikka-animate-booth";

#[derive(Debug)]
pub struct Booth {
    queue: Queue<Task>,
    studio: Sender<Message>,
    thread: Option<JoinHandle<()>>,
    _pool: Pool<Task>,
}

impl Booth {
    pub fn new() -> Result<Self, BoothError> {
        let (studio, inbox) = mpsc::channel();
        let loader = Loader::new();
        let delivering = studio.clone();
        let pool = Pool::new(LOADER_THREAD, LOADERS, move |task| {
            work::run(&loader, &delivering, task);
        })
        .map_err(BoothError::Spawn)?;
        let queue = pool.queue();
        let preparing = pool.queue();
        let rouse = studio.clone();
        let thread = std::thread::Builder::new()
            .name(STUDIO_THREAD.to_owned())
            .spawn(move || studio::run(preparing, rouse, inbox))
            .map_err(BoothError::Spawn)?;
        Ok(Self {
            queue,
            studio,
            thread: Some(thread),
            _pool: pool,
        })
    }

    pub fn render(
        &self,
        order: Order,
        done: impl FnOnce(Result<Picture, BoothError>) + Send + 'static,
    ) -> Ticket {
        let (delivery, ticket) = Delivery::still(Box::new(done));
        self.order(order, delivery);
        ticket
    }

    pub fn preview(&self, order: Order, feed: Feed) -> Preview {
        let (delivery, preview) = Delivery::live(feed);
        self.order(order, delivery);
        preview
    }

    fn order(&self, order: Order, delivery: Delivery) {
        self.queue.push(Task::Load(Job {
            sitter: order.sitter,
            size: order.size,
            delivery,
        }));
    }
}

impl Drop for Booth {
    fn drop(&mut self) {
        let _ = self.studio.send(Message::Stop);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("booth studio panicked");
        }
    }
}
