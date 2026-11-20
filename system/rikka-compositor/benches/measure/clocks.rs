use std::io::{BufRead as _, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Sample {
    pub(crate) at: Instant,
    pub(crate) graphics: u32,
    pub(crate) memory: u32,
    pub(crate) used: u32,
    pub(crate) pstate: u32,
}

pub(crate) struct Clocks {
    child: Option<Child>,
    samples: Arc<Mutex<Vec<Sample>>>,
}

impl Clocks {
    pub(crate) fn watch() -> Self {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let child = Command::new("nvidia-smi")
            .args([
                "--query-gpu=clocks.gr,clocks.mem,memory.used,pstate",
                "--format=csv,noheader,nounits",
                "-lms",
                "20",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok();
        let child = child.map(|mut child| {
            let stdout = child.stdout.take().expect("stdout was piped");
            let shared_samples = Arc::clone(&samples);
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let mut numbers = line
                        .split(',')
                        .map(|field| field.trim().trim_start_matches('P').parse::<u32>());
                    if let (
                        Some(Ok(graphics)),
                        Some(Ok(memory)),
                        Some(Ok(used)),
                        Some(Ok(pstate)),
                    ) = (
                        numbers.next(),
                        numbers.next(),
                        numbers.next(),
                        numbers.next(),
                    ) {
                        shared_samples.lock().unwrap().push(Sample {
                            at: Instant::now(),
                            graphics,
                            memory,
                            used,
                            pstate,
                        });
                    }
                }
            });
            child
        });
        Self { child, samples }
    }

    pub(crate) fn taken(&self) -> Vec<Sample> {
        self.samples.lock().unwrap().clone()
    }
}

impl Drop for Clocks {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
