use std::hint::black_box;

use divan::Bencher;
use divan::counter::{BytesCount, ItemsCount};
use ipc_bench::{DBus, Seam, Socket, Transport, payload};

const PAYLOADS: [usize; 3] = [16, 4096, 65536];
const PIPELINED: usize = 32;

fn main() {
    divan::main();
}

#[divan::bench(types = [Seam, DBus, Socket], args = PAYLOADS)]
fn round_trip<T: Transport>(bencher: Bencher, bytes: usize) {
    let server = T::serve();
    let mut transport = T::connect(&server);
    let text = payload(bytes);
    bencher
        .counter(BytesCount::new(2 * bytes))
        .bench_local(|| transport.echo(black_box(&text)));
}

#[divan::bench(types = [Seam, DBus, Socket])]
fn pipelined<T: Transport>(bencher: Bencher) {
    let server = T::serve();
    let mut transport = T::connect(&server);
    let text = payload(PAYLOADS[0]);
    bencher
        .counter(ItemsCount::new(PIPELINED))
        .bench_local(|| transport.echo_many(black_box(&text), PIPELINED));
}

#[divan::bench(types = [Seam, DBus, Socket])]
fn connect<T: Transport>(bencher: Bencher) {
    let server = T::serve();
    bencher.bench_local(|| T::connect(&server));
}
