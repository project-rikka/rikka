use rikka_gpu::Download;

pub fn words<T: bytemuck::Pod, E: From<rikka_gpu::Error>>(
    download: Download,
    expected: u64,
    short: impl FnOnce(usize) -> E,
) -> Result<Vec<T>, E> {
    let bytes = download.wait()?;
    let Some(prefix) = bytes.get(..expected as usize) else {
        return Err(short(bytes.len()));
    };
    Ok(bytemuck::pod_collect_to_vec(prefix))
}
