use super::*;
use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test runtime")
}

async fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("isolated fixture");
    let source = temp.path().join("synthetic.edb");
    std::fs::write(&source, b"Synthetic placeholder, not an ETABS database")
        .expect("source fixture");
    let project = temp.path().join("project");
    ext_api::init::init_project(ext_api::init::InitRequest {
        name: "Synthetic native API fixture".into(),
        edb_path: source,
        project_path: project.clone(),
        author: Some("Synthetic Fixture".into()),
        email: Some("fixture@example.invalid".into()),
        one_drive_dir: None,
        reports_dir: None,
        allow_onedrive: true,
    })
    .await
    .expect("initialize only disposable fixture");
    let mut context = AppContext::new_read_only(&project).expect("fixture context");
    context.set_sidecar(None);
    for message in ["Synthetic v1", "Synthetic v2"] {
        ext_api::commit::commit_version(
            &context,
            message,
            ext_api::commit::CommitOptions {
                no_e2k: true,
                analyze: false,
            },
        )
        .await
        .expect("fixture-only commit, no sidecar");
    }
    temp
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    fn walk(root: &Path, path: &Path, output: &mut BTreeMap<PathBuf, (Vec<u8>, SystemTime)>) {
        for entry in std::fs::read_dir(path).expect("read fixture tree") {
            let path = entry.expect("fixture entry").path();
            if path.is_dir() {
                walk(root, &path, output);
            } else {
                output.insert(
                    path.strip_prefix(root)
                        .expect("relative fixture path")
                        .to_owned(),
                    (
                        std::fs::read(&path).expect("fixture bytes"),
                        std::fs::metadata(&path)
                            .expect("fixture metadata")
                            .modified()
                            .expect("mtime"),
                    ),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}

fn json(value: &impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(value).expect("authoritative DTO serializes")
}

#[test]
fn reads_match_authoritative_api_and_preserve_all_fixture_bytes_and_mtimes() {
    runtime().block_on(async {
        let temp = fixture().await;
        let project = temp.path().join("project");
        let partial = project.join(".etabs-ext/main/v999");
        std::fs::create_dir(&partial).expect("partial snapshot fixture");
        std::fs::write(partial.join(".partial"), b"must not be cleaned by reads")
            .expect("sentinel");
        let before = snapshot(&project);
        let mut reference = AppContext::new_read_only(&project).expect("reference context");
        reference.set_sidecar(None);

        let BackendPayload::Project(actual) = execute_read(BackendRequest::project(&project))
            .await
            .expect("read project")
        else {
            panic!("wrong DTO")
        };
        assert_eq!(
            json(&actual.status),
            json(
                &project_status(&reference, StatusOptions { verbose: false })
                    .await
                    .expect("reference status")
            )
        );
        assert_eq!(
            json(&actual.branches),
            json(&list_branches(&reference).await.expect("reference branches"))
        );
        let BackendPayload::Version(actual) =
            execute_read(BackendRequest::version(&project, "main", "v2"))
                .await
                .expect("read version")
        else {
            panic!("wrong DTO")
        };
        assert_eq!(
            json(&actual.versions),
            json(
                &list_versions(&reference, Some("main"), false)
                    .await
                    .expect("reference versions")
            )
        );
        assert_eq!(
            json(&actual.detail),
            json(
                &show_version(&reference, "main/v2")
                    .await
                    .expect("reference detail")
            )
        );
        assert_eq!(
            json(&actual.overview),
            json(&get_project_overview(&reference, "main/v2").expect("reference overview"))
        );
        let mut worker = NativeBackend::new().expect("real read worker");
        let ticket = worker
            .submit(BackendRequest::version(&project, "main", "v2"))
            .expect("submit real read");
        let completed = next_completion(&worker);
        assert_eq!(completed.load.generation, ticket.generation);
        let BackendOutcome::Current(Ok(BackendPayload::Version(worker_actual))) = completed.outcome
        else {
            panic!("expected real worker DTO")
        };
        assert_eq!(json(&worker_actual.detail), json(&actual.detail));
        let BackendPayload::Diff(actual) =
            execute_read(BackendRequest::diff(&project, "main", "v1", "v2"))
                .await
                .expect("read diff")
        else {
            panic!("wrong DTO")
        };
        assert_eq!(
            json(&actual.diff),
            json(
                &diff_versions(&reference, "main/v1", "main/v2")
                    .await
                    .expect("reference diff")
            )
        );
        assert!(actual.diff.no_e2k_warning.is_some());
        assert_eq!(
            before,
            snapshot(&project),
            "reads must preserve even partial snapshots and .git files"
        );
    });
}

#[test]
fn error_categories_follow_typed_causes_instead_of_similar_prose() {
    let busy = BackendError::from_backend(
        "fixture",
        AnyError::new(ExtError::SidecarBusy {
            command: "synthetic".into(),
            waited_ms: 5,
        }),
    );
    assert_eq!(busy.kind(), BackendErrorKind::Busy);
    let missing = BackendError::from_backend(
        "fixture",
        AnyError::new(ExtError::VersionNotFound("v404".into())),
    );
    assert_eq!(missing.kind(), BackendErrorKind::NotFound);
    let ambiguous_prose = BackendError::from_backend(
        "fixture",
        AnyError::new(ExtError::Other("Version not found; Sidecar is busy".into())),
    );
    assert_eq!(ambiguous_prose.kind(), BackendErrorKind::Backend);
    assert!(
        ambiguous_prose
            .source_chain()
            .iter()
            .any(|source| source.contains("Sidecar is busy"))
    );
}

#[test]
fn invalid_paths_refs_and_corrupt_state_have_typed_errors_and_no_writes() {
    runtime().block_on(async {
        let temp = fixture().await;
        let project = temp.path().join("project");
        assert_eq!(
            execute_read(BackendRequest::project(temp.path().join("missing")))
                .await
                .expect_err("missing")
                .kind(),
            BackendErrorKind::NotFound
        );
        assert_eq!(
            execute_read(BackendRequest::project(temp.path()))
                .await
                .expect_err("invalid project")
                .kind(),
            BackendErrorKind::InvalidProject
        );
        let before = snapshot(&project);
        for request in [
            BackendRequest::version(&project, "main", "v404"),
            BackendRequest::version(&project, "../outside", "v1"),
            BackendRequest::version(&project, "main", "../../outside"),
        ] {
            assert_eq!(
                execute_read(request)
                    .await
                    .expect_err("reject unknown reference")
                    .kind(),
                BackendErrorKind::NotFound
            );
        }
        assert_eq!(snapshot(&project), before);
        std::fs::write(project.join(".etabs-ext/state.json"), b"{broken")
            .expect("corrupt isolated state");
        let before = snapshot(&project);
        let error = execute_read(BackendRequest::project(&project))
            .await
            .expect_err("corrupt state");
        assert_eq!(error.kind(), BackendErrorKind::InvalidProject);
        assert!(
            error.source_chain().len() >= 2,
            "retain operation context and the underlying state error"
        );
        assert_eq!(snapshot(&project), before);
    });
}

fn next_completion(backend: &NativeBackend) -> BackendCompletion {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = backend.try_recv().expect("live worker") {
            return value;
        }
        assert!(Instant::now() < deadline, "worker completion deadline");
        thread::sleep(Duration::from_millis(1));
    }
}

fn synthetic_error() -> BackendResult<BackendPayload> {
    Err(BackendError::new(
        BackendErrorKind::NotFound,
        "synthetic read",
        anyhow!("fixture outcome"),
    ))
}

#[test]
fn out_of_order_project_completion_never_exposes_an_old_payload() {
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let release = Mutex::new(release_recv);
    let executor: Arc<Executor> = Arc::new(move |request: BackendRequest| {
        if request.project_root == Path::new("old-project") {
            started_send.send(()).expect("test synchronization");
            release
                .lock()
                .expect("test lock")
                .recv()
                .expect("release old request");
        }
        synthetic_error()
    });
    let mut backend = NativeBackend::with_executor(2, &executor).expect("worker");
    let old = backend
        .submit(BackendRequest::project("old-project"))
        .expect("old request");
    started_recv
        .recv_timeout(Duration::from_secs(10))
        .expect("old request started");
    let new = backend
        .submit(BackendRequest::project("new-project"))
        .expect("new request");
    let current = next_completion(&backend);
    assert_eq!(current.load.generation, new.generation);
    assert_eq!(current.project_root, Path::new("new-project"));
    assert!(matches!(current.outcome, BackendOutcome::Current(_)));
    release_send.send(()).expect("release");
    let stale = next_completion(&backend);
    assert_eq!(stale.load.generation, old.generation);
    assert!(matches!(stale.outcome, BackendOutcome::Superseded));
}

#[test]
fn a_busy_selection_also_invalidates_previously_accepted_results() {
    let (started_send, started_recv) = mpsc::channel();
    let (release_send, release_recv) = mpsc::channel();
    let release = Mutex::new(release_recv);
    let executor: Arc<Executor> = Arc::new(move |_: BackendRequest| {
        started_send.send(()).expect("test synchronization");
        release
            .lock()
            .expect("test lock")
            .recv()
            .expect("release request");
        synthetic_error()
    });
    let mut backend = NativeBackend::with_executor(1, &executor).expect("worker");
    backend
        .submit(BackendRequest::project("first"))
        .expect("first request");
    started_recv
        .recv_timeout(Duration::from_secs(10))
        .expect("started");
    backend
        .submit(BackendRequest::project("queued"))
        .expect("queue slot");
    assert_eq!(
        backend
            .submit(BackendRequest::project("active-but-busy"))
            .expect_err("bounded queue")
            .kind(),
        BackendErrorKind::Busy
    );
    release_send.send(()).expect("release first");
    assert!(matches!(
        next_completion(&backend).outcome,
        BackendOutcome::Superseded
    ));
    started_recv
        .recv_timeout(Duration::from_secs(10))
        .expect("queued request started");
    release_send.send(()).expect("release queued");
    assert!(matches!(
        next_completion(&backend).outcome,
        BackendOutcome::Superseded
    ));
}
