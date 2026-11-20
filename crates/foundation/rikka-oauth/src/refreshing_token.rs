use std::time::Duration;

use tokio::sync::RwLock;

use crate::token::now;

const REFRESH_MARGIN: Duration = Duration::from_secs(300);

pub trait Refresh: Clone + Send + Sync + 'static {
    type Error: Send;

    fn expires_at(&self) -> Option<u64>;

    fn refresh(&self) -> impl Future<Output = Result<Self, Self::Error>> + Send;
}

type RefreshCallback<T> = Box<dyn Fn(&T) + Send + Sync>;

pub struct RefreshingToken<T> {
    state: RwLock<State<T>>,
    callback: Option<RefreshCallback<T>>,
}

struct State<T> {
    tokens: T,
    refreshed: bool,
}

impl<T: Refresh> RefreshingToken<T> {
    #[must_use]
    pub fn new(tokens: T) -> RefreshingToken<T> {
        RefreshingToken {
            state: RwLock::new(State {
                tokens,
                refreshed: false,
            }),
            callback: None,
        }
    }

    #[must_use]
    pub fn on_refresh(
        mut self,
        callback: impl Fn(&T) + Send + Sync + 'static,
    ) -> RefreshingToken<T> {
        self.callback = Some(Box::new(callback));
        self
    }

    pub async fn current(&self) -> Result<T, T::Error> {
        {
            let state = self.state.read().await;
            if state.is_fresh() {
                return Ok(state.tokens.clone());
            }
        }

        let refreshed = {
            let mut state = self.state.write().await;
            if state.is_fresh() {
                return Ok(state.tokens.clone());
            }

            state.tokens = state.tokens.refresh().await?;
            state.refreshed = true;
            state.tokens.clone()
        };

        if let Some(callback) = &self.callback {
            callback(&refreshed);
        }
        Ok(refreshed)
    }
}

impl<T: Refresh> State<T> {
    fn is_fresh(&self) -> bool {
        match self.tokens.expires_at() {
            Some(expires_at) => now() + REFRESH_MARGIN.as_secs() < expires_at,
            None => self.refreshed,
        }
    }
}

impl<T> std::fmt::Debug for RefreshingToken<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RefreshingToken")
            .finish_non_exhaustive()
    }
}
