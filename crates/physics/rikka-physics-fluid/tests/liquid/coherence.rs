use glam::Vec3;
use rikka_physics_fluid::{Emission, Fluid, Inputs, Nozzle, Particle, Substance};

const RADIUS: f32 = 0.002;
const REACH: f32 = 1.7 * 2.0 * RADIUS;

fn fluid(substance: Substance) -> Fluid {
    let mut fluid = crate::support::fluid(
        Fluid::builder()
            .radius(RADIUS)
            .capacity(1 << 14)
            .spray_capacity(2048)
            .floor(0.0),
    );
    fluid.set_substances(&[substance]).expect("a substance");
    fluid
}

#[test]
fn falling_stream_stays_connected_for_30_cm() {
    let mut fluid = fluid(crate::support::water());
    let mouth = Vec3::new(0.0, 0.5, 0.0);
    let speed = 1.0;
    let radius = 0.008;
    let stream = Emission::new(0, Nozzle::Stream { radius })
        .at(mouth)
        .velocity(Vec3::new(0.0, -speed, 0.0))
        .rate(std::f32::consts::PI * radius * radius * speed);
    for frame in 0..=24u32 {
        fluid
            .advance_to(f64::from(frame) / 60.0, &Inputs::new().emit(stream.clone()))
            .expect("a frame");
    }
    let particles = fluid.read_particles().expect("particles");
    let joined = attached(&particles, |particle| particle.position.y > mouth.y - 0.01);
    let reach = joined
        .iter()
        .map(|particle| mouth.y - particle.position.y)
        .fold(0.0, f32::max);
    assert!(reach > 0.3, "the stream holds together for only {reach} m");
}

fn attached(particles: &[Particle], from: impl Fn(&Particle) -> bool) -> Vec<Particle> {
    let mut connected = vec![false; particles.len()];
    let mut queue: Vec<usize> = (0..particles.len())
        .filter(|&at| from(&particles[at]))
        .collect();
    for &at in &queue {
        connected[at] = true;
    }
    let mut joined = Vec::new();
    while let Some(at) = queue.pop() {
        joined.push(particles[at]);
        for (other, particle) in particles.iter().enumerate() {
            if !connected[other] && particle.position.distance(particles[at].position) < REACH {
                connected[other] = true;
                queue.push(other);
            }
        }
    }
    joined
}
