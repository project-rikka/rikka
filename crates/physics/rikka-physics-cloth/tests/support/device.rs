use rikka_sync::{Mutex, MutexGuard};
use std::sync::LazyLock;

use rikka_gpu::{Compute, Gpu};
use rikka_physics_cloth::{Cloth, ClothBuilder};

static TURN: Mutex<()> = Mutex::new(());

struct Device {
    compute: Compute,
    _kernels: Mutex<Cloth>,
}

static DEVICE: LazyLock<Device> = LazyLock::new(|| {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let compute = Gpu::builder()
        .app_name("rikka-physics-cloth")
        .compute_support()
        .build()
        .expect("a Vulkan 1.3 device that can compute is required")
        .compute()
        .expect("compute");
    let kernels = Cloth::builder().build(&compute).expect("a cloth");
    Device {
        compute,
        _kernels: Mutex::new(kernels),
    }
});

thread_local! {
    static TURN_HELD: MutexGuard<'static, ()> = TURN.lock();
}

pub(crate) fn compute() -> Compute {
    TURN_HELD.with(|_| ());
    DEVICE.compute.clone()
}

pub(crate) fn cloth(builder: ClothBuilder) -> Cloth {
    builder.build(&compute()).expect("a cloth")
}
