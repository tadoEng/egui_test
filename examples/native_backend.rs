//! Explicit read-only project probe. No project is opened by default.

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use eframe_template::native_backend::{BackendOutcome, BackendRequest, NativeBackend};
    env_logger::init();
    let mut arguments = std::env::args_os().skip(1);
    let project = arguments
        .next()
        .ok_or("provide an explicit disposable project path")?;
    if arguments.next().is_some() {
        return Err("expected one disposable project path".into());
    }
    let mut backend = NativeBackend::new()?;
    let ticket = backend.submit(BackendRequest::project(project))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some(completion) = backend.try_recv()? {
            match completion.outcome {
                BackendOutcome::Current(Ok(_)) => {
                    log::info!(
                        "read-only project probe complete; generation {}",
                        ticket.generation
                    );
                    return Ok(());
                }
                BackendOutcome::Current(Err(error)) => return Err(error.into()),
                BackendOutcome::Superseded => return Err("request superseded".into()),
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err("read-only probe timed out; no external process was started".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
