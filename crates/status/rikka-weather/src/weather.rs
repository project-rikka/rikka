use std::pin::Pin;
use std::sync::Arc;

use futures_util::Stream;
use rikka_worker::Observable;
use rikka_worker::{Stop, WallAlarm, Worker};
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;

use crate::error::Error;
use crate::forecaster::Forecaster;
use crate::place::{Place, Spot};
use crate::report::Report;

const NAME: &str = "rikka-weather";

pub(crate) type Online = Pin<Box<dyn Stream<Item = bool> + Send>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WeatherError {
    NoLocation,
    Service(Error),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub spot: Spot,
    pub place: Option<Place>,
    pub report: Option<Arc<Report>>,
    pub refreshing: bool,
    pub error: Option<WeatherError>,
}

pub(crate) enum Order {
    Spot(Spot),
    Refresh,
    Stop,
}

struct Orders(mpsc::UnboundedSender<Order>);

impl Stop for Orders {
    fn stop(&self) {
        if self.0.send(Order::Stop).is_err() {
            tracing::trace!("weather worker already stopped");
        }
    }
}

#[derive(Clone)]
pub struct Weather(Arc<Shared>);

struct Shared {
    state: Observable<State>,
    orders: Option<mpsc::UnboundedSender<Order>>,
    _worker: Option<Worker<Orders>>,
}

#[must_use]
pub struct WeatherBuilder {
    app: String,
    spot: Spot,
    online: Option<Online>,
}

impl WeatherBuilder {
    pub fn spot(mut self, spot: Spot) -> Self {
        self.spot = spot;
        self
    }

    pub fn online(mut self, online: impl Stream<Item = bool> + Send + 'static) -> Self {
        self.online = Some(Box::pin(online));
        self
    }

    #[must_use]
    pub fn start(self) -> Weather {
        let WeatherBuilder { app, spot, online } = self;
        let state = Observable::new(State {
            spot: spot.clone(),
            ..State::default()
        });
        let (orders, receiver) = mpsc::unbounded_channel();
        let publishing = state.clone();
        let worker = Worker::spawn(NAME, Orders(orders.clone()), move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?;
            let alarm = {
                let _inside = runtime.enter();
                WallAlarm::new().and_then(AsyncFd::new)
            }
            .map_err(|error| format!("the wall clock cannot be followed: {error}"))?;
            let forecaster = Forecaster::new(app, publishing, alarm, online)
                .map_err(|error| error.to_string())?;
            Ok(move || runtime.block_on(forecaster.run(receiver, spot)))
        })
        .inspect_err(|error: &rikka_worker::Error<String>| {
            tracing::warn!("failed to start weather updates: {error}");
        })
        .ok();
        Weather(Arc::new(Shared {
            state,
            orders: worker.is_some().then_some(orders),
            _worker: worker,
        }))
    }
}

impl std::fmt::Debug for WeatherBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeatherBuilder")
            .field("app", &self.app)
            .field("spot", &self.spot)
            .field("online", &self.online.is_some())
            .finish()
    }
}

impl Weather {
    pub fn builder(app_id: impl Into<String>) -> WeatherBuilder {
        WeatherBuilder {
            app: app_id.into(),
            spot: Spot::Here,
            online: None,
        }
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.0.state.get()
    }

    pub fn changes(&self) -> impl Stream<Item = State> + Send + 'static {
        self.0.state.changes()
    }

    pub fn set_spot(&self, spot: Spot) {
        if self.0.state.with(|state| state.spot == spot) {
            return;
        }
        self.order(Order::Spot(spot));
    }

    pub fn refresh(&self) {
        self.order(Order::Refresh);
    }

    fn order(&self, order: Order) {
        if let Some(orders) = &self.0.orders
            && orders.send(order).is_err()
        {
            tracing::debug!("weather request dropped: worker stopped");
        }
    }
}

impl std::fmt::Debug for Weather {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.state.with(|state| {
            f.debug_struct("Weather")
                .field("spot", &state.spot)
                .field("place", &state.place)
                .field("refreshing", &state.refreshing)
                .finish_non_exhaustive()
        })
    }
}
