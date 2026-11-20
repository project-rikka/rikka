use render_bench::{Counting, GpuTimes, Pacing, Scenario, measure};
use rikka_renderer::Renderer;

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const BENCH_FRAMES: usize = 900;

const SMOKE_FRAMES: usize = 6;

const DISPLAY_HZ: u32 = 180;

fn main() {
    let benching = std::env::args().any(|argument| argument == "--bench");
    let rate = std::env::args()
        .find_map(|argument| argument.strip_prefix("--hz=")?.parse().ok())
        .unwrap_or(DISPLAY_HZ);
    let frames = std::env::args()
        .find_map(|argument| argument.strip_prefix("--frames=")?.parse().ok())
        .unwrap_or(match benching {
            true => BENCH_FRAMES,
            false => SMOKE_FRAMES,
        });
    let filters: Vec<String> = std::env::args()
        .skip(1)
        .filter(|argument| !argument.starts_with("--"))
        .collect();
    let times = match std::env::args().any(|argument| argument == "--untimed") {
        true => GpuTimes::default(),
        false => GpuTimes::install(),
    };
    let mut renderer = Renderer::new(
        &rikka_gpu::Gpu::builder()
            .app_name("render-bench")
            .ray_tracing()
            .build()
            .expect("Vulkan 1.3 device"),
    )
    .expect("Vulkan 1.3 device");
    for scenario in Scenario::ALL {
        if !filters.is_empty()
            && !filters
                .iter()
                .any(|name| scenario.name().contains(name.as_str()))
        {
            continue;
        }
        if scenario.traces_rays() && !renderer.gpu().traces_rays() {
            println!("{}: skipped, this GPU cannot trace rays", scenario.name());
            continue;
        }
        let measured = measure(&mut renderer, &times, scenario, frames, Pacing::hz(rate));
        assert_eq!(
            measured.frames(),
            frames,
            "{} drew too few frames",
            scenario.name()
        );
        println!("{measured}");
        if let Some(allocated) = measured.steady_allocations() {
            assert_eq!(
                allocated,
                0,
                "{} allocates in steady frames:\n{}",
                scenario.name(),
                measured.steady_traces().join("\n----\n")
            );
        }
    }
}
