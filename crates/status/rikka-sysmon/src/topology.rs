use crate::reading;

pub(crate) const CPU_DIR: &str = "/sys/devices/system/cpu";

#[derive(Clone, Debug, Default)]
pub struct Topology {
    pub model: String,
    pub base_ghz: f32,
    pub sockets: usize,
    pub cores: usize,
    pub logical: usize,
    pub virtualization: bool,
    pub caches: Caches,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Caches {
    pub l1: u64,
    pub l2: u64,
    pub l3: u64,
}

impl Topology {
    #[must_use]
    pub fn read() -> Self {
        let cpuinfo = reading::text("/proc/cpuinfo").unwrap_or_default();
        let logical = processors().len().max(1);
        let (sockets, cores) = packages(&cpuinfo, logical);
        let model = reading::field(&cpuinfo, "model name")
            .unwrap_or("Unknown processor")
            .to_owned();
        Self {
            base_ghz: base_ghz(&model),
            sockets,
            cores,
            logical,
            virtualization: virtualization(&cpuinfo),
            caches: Caches::read(),
            model,
        }
    }
}

impl Caches {
    fn read() -> Self {
        let mut caches = Self::default();
        let mut counted: Vec<(String, u32)> = Vec::new();
        for cpu in processors() {
            let dir = format!("{CPU_DIR}/{cpu}/cache");
            for index in reading::entries(&dir) {
                let index = format!("{dir}/{index}");
                let Some(level) = reading::number::<u32>(format!("{index}/level")) else {
                    continue;
                };
                let shared = reading::text(format!("{index}/shared_cpu_list")).unwrap_or_default();
                let Some(size) =
                    reading::text(format!("{index}/size")).and_then(|size| kibibytes(&size))
                else {
                    continue;
                };
                let kind = reading::text(format!("{index}/type")).unwrap_or_default();
                let key = (format!("{shared}/{kind}"), level);
                if counted.contains(&key) {
                    continue;
                }
                counted.push(key);
                match level {
                    1 => caches.l1 += size,
                    2 => caches.l2 += size,
                    3 => caches.l3 += size,
                    _ => {}
                }
            }
        }
        caches
    }
}

fn kibibytes(size: &str) -> Option<u64> {
    let digits: String = size.chars().take_while(char::is_ascii_digit).collect();
    let value: u64 = digits.parse().ok()?;
    Some(match size.trim().chars().last()? {
        'K' => value * 1024,
        'M' => value * 1024 * 1024,
        _ => value,
    })
}

pub(crate) fn processors() -> Vec<String> {
    let mut found: Vec<String> = reading::entries(CPU_DIR)
        .into_iter()
        .filter(|name| {
            name.strip_prefix("cpu").is_some_and(|rest| {
                !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit())
            })
        })
        .collect();
    found.sort();
    found
}

fn packages(cpuinfo: &str, logical: usize) -> (usize, usize) {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for cpu in processors() {
        let dir = format!("{CPU_DIR}/{cpu}/topology");
        let Some(package) = reading::text(format!("{dir}/physical_package_id")) else {
            continue;
        };
        let core = reading::text(format!("{dir}/core_id")).unwrap_or_default();
        let pair = (package, core);
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }
    if pairs.is_empty() {
        let cores = reading::field(cpuinfo, "cpu cores")
            .and_then(|cores| cores.parse().ok())
            .unwrap_or(logical);
        return (1, cores.max(1));
    }
    let mut sockets: Vec<&String> = pairs.iter().map(|(package, _)| package).collect();
    sockets.sort_unstable();
    sockets.dedup();
    (sockets.len().max(1), pairs.len())
}

fn base_ghz(model: &str) -> f32 {
    if let Some(base) = reading::number::<f32>(format!("{CPU_DIR}/cpu0/cpufreq/base_frequency")) {
        return base / 1_000_000.0;
    }
    model
        .rsplit_once('@')
        .and_then(|(_, clock)| clock.trim().strip_suffix("GHz"))
        .and_then(|clock| clock.trim().parse().ok())
        .unwrap_or_default()
}

fn virtualization(cpuinfo: &str) -> bool {
    reading::field(cpuinfo, "flags").is_some_and(|flags| {
        flags
            .split_whitespace()
            .any(|flag| flag == "vmx" || flag == "svm")
    })
}
