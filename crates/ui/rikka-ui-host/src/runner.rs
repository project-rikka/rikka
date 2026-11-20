use std::sync::Arc;

use futures_util::stream::StreamExt;
use tokio::task::JoinHandle;

use rikka_ui::{JobResult, RuntimeMessage, Work};

pub trait TaskSink<K>: Send + Sync + 'static {
    fn deliver(&self, key: K, inbound: RuntimeMessage) -> bool;
}

pub(crate) struct TaskRunner<K> {
    sink: Arc<dyn TaskSink<K>>,
}

impl<K: Send + Clone + 'static> TaskRunner<K> {
    pub(crate) fn new(sink: impl TaskSink<K>) -> Self {
        Self {
            sink: Arc::new(sink),
        }
    }

    pub(crate) fn spawn(&self, key: K, job: u64, work: Work) -> JoinHandle<()> {
        let sink = Arc::clone(&self.sink);
        let runtime = rikka_ui::work_runtime();
        let job_result = move |output| RuntimeMessage::JobResult(JobResult { job, output });
        match work {
            Work::Future(future) => runtime.spawn(async move {
                let output = future.await;
                sink.deliver(key, job_result(Some(output)));
            }),
            Work::Stream(mut stream) => runtime.spawn(async move {
                while let Some(output) = stream.next().await {
                    if !sink.deliver(key.clone(), job_result(Some(output))) {
                        return;
                    }
                }
                sink.deliver(key, job_result(None));
            }),
            Work::Blocking(work) => runtime.spawn_blocking(move || {
                sink.deliver(key, job_result(Some(work())));
            }),
        }
    }
}

impl<K> std::fmt::Debug for TaskRunner<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskRunner").finish_non_exhaustive()
    }
}
