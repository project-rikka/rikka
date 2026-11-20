use tokio::task::JoinHandle;

use rikka_ui::Job;

use crate::runner::TaskRunner;

struct Running {
    name: Option<u64>,
    handle: JoinHandle<()>,
}

#[derive(Default)]
pub(crate) struct Tasks {
    running: Vec<Running>,
}

impl Tasks {
    pub(crate) fn apply<K: Send + Clone + 'static>(
        &mut self,
        jobs: Vec<Job>,
        runner: &TaskRunner<K>,
        key: &K,
    ) {
        if jobs.is_empty() {
            return;
        }
        self.running.retain(|running| !running.handle.is_finished());
        for job in jobs {
            match job {
                Job::Cancel(name) => self.stop(name),
                Job::Start {
                    id,
                    key: name,
                    work,
                } => {
                    let handle = runner.spawn(key.clone(), id, work);
                    self.running.push(Running { name, handle });
                }
            }
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.running.len()
    }

    fn stop(&mut self, name: u64) {
        self.running.retain(|running| {
            if running.name != Some(name) {
                return true;
            }
            running.handle.abort();
            false
        });
    }
}

impl Drop for Tasks {
    fn drop(&mut self) {
        for running in &self.running {
            running.handle.abort();
        }
    }
}
