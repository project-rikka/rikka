pub(crate) fn name() -> std::io::Result<String> {
    Ok(rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .into_owned())
}
