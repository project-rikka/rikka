use rikka_physics_collision::{Broadphase, Filter, ProxyKind, pair_of};
use rikka_physics_core::{Executor, SerialExecutor, WorkerPool};
use rikka_physics_math::{Aabb, Vec3};

struct Random(u64);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }

    fn unit(&mut self) -> f32 {
        (self.next() & 0xffff) as f32 / 65535.0
    }

    fn bounds(&mut self, extent: f32) -> Aabb {
        let center = Vec3::new(self.unit(), self.unit(), self.unit()) * extent;
        let half = Vec3::splat(0.2 + 0.8 * self.unit());
        Aabb::new(center - half, center + half)
    }
}

const PROXIES: u32 = 700;
const EXTENT: f32 = 24.0;

fn kind_of(proxy: u32) -> ProxyKind {
    match proxy % 10 {
        0 => ProxyKind::Static,
        1 => ProxyKind::Kinematic,
        _ => ProxyKind::Dynamic,
    }
}

fn run(executor: &dyn Executor) -> Vec<Vec<u64>> {
    let mut random = Random(7);
    let mut broadphase = Broadphase::new();
    let mut bounds = vec![Aabb::EMPTY; PROXIES as usize];
    let mut alive = vec![false; PROXIES as usize];
    let allow = |_: u32, _: u32| true;
    let mut history = Vec::new();
    for step in 0..60 {
        for proxy in 0..PROXIES {
            let index = proxy as usize;
            let roll = random.unit();
            if !alive[index] {
                if step == 0 || roll < 0.05 {
                    bounds[index] = random.bounds(EXTENT);
                    broadphase.create(
                        proxy,
                        kind_of(proxy),
                        bounds[index],
                        Filter::default(),
                        proxy,
                        true,
                    );
                    alive[index] = true;
                }
            } else if roll < 0.02 {
                broadphase.destroy(proxy);
                alive[index] = false;
                bounds[index] = Aabb::EMPTY;
                for other in 0..PROXIES {
                    broadphase.remove_pair(proxy, other);
                }
            } else if kind_of(proxy) != ProxyKind::Static && roll < 0.5 || roll < 0.03 {
                let shift = Vec3::new(random.unit(), random.unit(), random.unit()) - 0.5;
                bounds[index] = bounds[index].translated(shift * 2.0);
                broadphase.move_proxy(proxy, bounds[index]);
            }
        }
        let created = broadphase.update_pairs(executor, &allow).to_vec();
        for &key in &created {
            let (a, b) = pair_of(key);
            assert!(bounds[a as usize].overlaps(&bounds[b as usize]));
        }
        for a in 0..PROXIES {
            for b in a + 1..PROXIES {
                let dynamic = kind_of(a) == ProxyKind::Dynamic || kind_of(b) == ProxyKind::Dynamic;
                if alive[a as usize]
                    && alive[b as usize]
                    && dynamic
                    && bounds[a as usize].overlaps(&bounds[b as usize])
                {
                    assert!(broadphase.has_pair(a, b), "step {step} misses {a}-{b}");
                }
            }
        }
        history.push(created);
    }
    history
}

#[test]
fn finds_all_overlapping_pairs_for_any_worker_count() {
    let serial = run(&SerialExecutor);
    let pool = WorkerPool::builder().workers(4).build().unwrap();
    assert_eq!(serial, run(&pool));
}
