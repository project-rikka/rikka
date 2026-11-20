use ipc_bench::{DBus, Seam, Socket, Transport, payload};

fn echoes<T: Transport>() {
    let server = T::serve();
    for _ in 0..2 {
        let mut transport = T::connect(&server);
        for bytes in [0, 16, 65536] {
            let text = payload(bytes);
            assert_eq!(
                transport.echo(&text),
                text,
                "{} echoes {bytes} bytes",
                T::NAME
            );
        }
        assert_eq!(
            transport.echo_many(&payload(16), 32),
            32,
            "{} pipelines",
            T::NAME
        );
    }
}

#[test]
fn transports_echo_payloads() {
    echoes::<Seam>();
    echoes::<DBus>();
    echoes::<Socket>();
}
