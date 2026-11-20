pub trait Transport: Sized {
    const NAME: &'static str;

    type Server;

    fn serve() -> Self::Server;

    fn connect(server: &Self::Server) -> Self;

    fn echo(&mut self, text: &str) -> String;

    fn echo_many(&mut self, text: &str, count: usize) -> usize;
}

pub fn payload(bytes: usize) -> String {
    "seam".chars().cycle().take(bytes).collect()
}
