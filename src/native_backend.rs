//! Native-only, read-only access to authoritative `ext-api` project workflows.
//!
//! The adapter owns no engineering rules. It validates selections against
//! `ext-api` branch/version results, runs reads on a bounded worker pool, and
//! discards completions superseded by a newer project/version request.

use std::error::Error as StdError;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Error as AnyError, anyhow};
use ext_api::branch::{ListBranchesResult, list_branches};
use ext_api::diff::{DiffResult, diff_versions};
use ext_api::log::{ListVersionsResult, VersionDetail, list_versions, show_version};
use ext_api::status::{StatusOptions, StatusReport, project_status};
use ext_api::{
    AppContext, ProjectOverview, SceneLoadGuard, SceneLoadOperation, SceneLoadStatus,
    SceneLoadTicket, get_project_overview,
};
use ext_error::ExtError;

const QUEUE_CAPACITY: usize = 1;
const WORKER_COUNT: usize = 2;

/// Stable UI-facing error categories for the read-only backend boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendErrorKind {
    Busy,
    NotFound,
    InvalidProject,
    Unsupported,
    Backend,
}

/// A classified backend failure that retains the complete source error chain.
#[derive(Debug)]
pub struct BackendError {
    kind: BackendErrorKind,
    operation: &'static str,
    source: AnyError,
}

impl BackendError {
    fn new(kind: BackendErrorKind, operation: &'static str, source: AnyError) -> Self {
        Self {
            kind,
            operation,
            source,
        }
    }

    fn from_backend(operation: &'static str, source: AnyError) -> Self {
        let kind = classify_source(&source);
        Self::new(kind, operation, source)
    }

    pub fn kind(&self) -> BackendErrorKind {
        self.kind
    }

    pub fn operation(&self) -> &'static str {
        self.operation
    }

    /// Render every retained source in outermost-to-innermost order.
    pub fn source_chain(&self) -> Vec<String> {
        self.source.chain().map(ToString::to_string).collect()
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} while running {}: {}",
            self.kind, self.operation, self.source
        )
    }
}

impl StdError for BackendError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.source.as_ref())
    }
}

fn classify_source(error: &AnyError) -> BackendErrorKind {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<ExtError>())
        .map_or(BackendErrorKind::Backend, |source| match source {
            ExtError::SidecarBusy { .. } => BackendErrorKind::Busy,
            ExtError::VersionNotFound(_)
            | ExtError::BranchNotFound(_)
            | ExtError::FileNotFound(_) => BackendErrorKind::NotFound,
            ExtError::ConfigNotFound { .. }
            | ExtError::ConfigParse { .. }
            | ExtError::StateCorrupted(_)
            | ExtError::NotARepository { .. } => BackendErrorKind::InvalidProject,
            ExtError::InvalidUnitPreset { .. } => BackendErrorKind::Unsupported,
            _ => BackendErrorKind::Backend,
        })
}

pub type BackendResult<T> = Result<T, BackendError>;

/// One read operation accepted by [`NativeBackend`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOperation {
    Project,
    Version {
        branch: String,
        version: String,
    },
    Diff {
        branch: String,
        from_version: String,
        to_version: String,
    },
}

/// An explicit project path plus one read-only operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendRequest {
    project_root: PathBuf,
    operation: ReadOperation,
}

impl BackendRequest {
    pub fn project(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            operation: ReadOperation::Project,
        }
    }

    pub fn version(
        project_root: impl Into<PathBuf>,
        branch: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            operation: ReadOperation::Version {
                branch: branch.into(),
                version: version.into(),
            },
        }
    }

    pub fn diff(
        project_root: impl Into<PathBuf>,
        branch: impl Into<String>,
        from_version: impl Into<String>,
        to_version: impl Into<String>,
    ) -> Self {
        Self {
            project_root: project_root.into(),
            operation: ReadOperation::Diff {
                branch: branch.into(),
                from_version: from_version.into(),
                to_version: to_version.into(),
            },
        }
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn operation(&self) -> &ReadOperation {
        &self.operation
    }

    fn requested_ref(&self) -> String {
        match &self.operation {
            ReadOperation::Project => "project".to_owned(),
            ReadOperation::Version { branch, version } => format!("{branch}/{version}"),
            ReadOperation::Diff {
                branch,
                from_version,
                to_version,
            } => format!("{branch}/{from_version}..{to_version}"),
        }
    }
}

/// Authoritative project-level DTOs returned unchanged by `ext-api`.
#[derive(Debug)]
pub struct ProjectRead {
    pub status: StatusReport,
    pub branches: ListBranchesResult,
}

/// Authoritative branch/version DTOs returned unchanged by `ext-api`.
#[derive(Debug)]
pub struct VersionRead {
    pub branches: ListBranchesResult,
    pub versions: ListVersionsResult,
    pub detail: VersionDetail,
    pub overview: ProjectOverview,
}

/// Authoritative version validation and diff DTOs returned unchanged by `ext-api`.
#[derive(Debug)]
pub struct DiffRead {
    pub branches: ListBranchesResult,
    pub versions: ListVersionsResult,
    pub from_detail: VersionDetail,
    pub to_detail: VersionDetail,
    pub diff: DiffResult,
}

#[derive(Debug)]
pub enum BackendPayload {
    Project(Box<ProjectRead>),
    Version(Box<VersionRead>),
    Diff(Box<DiffRead>),
}

/// A current result, or proof that its payload was discarded as stale.
#[derive(Debug)]
pub enum BackendOutcome {
    Current(BackendResult<BackendPayload>),
    Superseded,
}

/// One settled completion carrying generation and requested-version identity.
#[derive(Debug)]
pub struct BackendCompletion {
    pub load: SceneLoadOperation,
    pub project_root: PathBuf,
    pub outcome: BackendOutcome,
}

struct WorkItem {
    ticket: SceneLoadTicket,
    request: BackendRequest,
}

struct WorkCompletion {
    ticket: SceneLoadTicket,
    project_root: PathBuf,
    result: BackendResult<BackendPayload>,
}

type Executor = dyn Fn(BackendRequest) -> BackendResult<BackendPayload> + Send + Sync + 'static;

/// Bounded native worker pool for read-only project/version operations.
pub struct NativeBackend {
    request_sender: SyncSender<WorkItem>,
    completion_receiver: Receiver<WorkCompletion>,
    load_guard: SceneLoadGuard,
}

impl NativeBackend {
    /// Start two workers with one additional queued request.
    ///
    /// # Errors
    /// Returns [`BackendErrorKind::Backend`] when a worker thread cannot start.
    pub fn new() -> BackendResult<Self> {
        let executor: Arc<Executor> = Arc::new(execute_blocking);
        Self::with_executor(WORKER_COUNT, &executor)
    }

    fn with_executor(worker_count: usize, executor: &Arc<Executor>) -> BackendResult<Self> {
        if worker_count == 0 {
            return Err(BackendError::new(
                BackendErrorKind::Unsupported,
                "start worker pool",
                anyhow!("at least one backend worker is required"),
            ));
        }

        let (request_sender, request_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let shared_receiver = Arc::new(Mutex::new(request_receiver));
        let (completion_sender, completion_receiver) =
            mpsc::sync_channel(WORKER_COUNT + QUEUE_CAPACITY);

        for index in 0..worker_count {
            let receiver = Arc::clone(&shared_receiver);
            let sender = completion_sender.clone();
            let executor = Arc::clone(executor);
            thread::Builder::new()
                .name(format!("native-backend-{index}"))
                .spawn(move || worker_loop(&receiver, &sender, executor.as_ref()))
                .map_err(|source| {
                    BackendError::new(
                        BackendErrorKind::Backend,
                        "start worker pool",
                        AnyError::new(source),
                    )
                })?;
        }
        drop(completion_sender);

        Ok(Self {
            request_sender,
            completion_receiver,
            load_guard: SceneLoadGuard::default(),
        })
    }

    /// Admit a request without blocking the UI thread.
    ///
    /// Every selection claims a generation, including a rejected busy request.
    /// Older results must not become current after the user has switched away.
    ///
    /// # Errors
    /// Returns Busy when the bounded queue is full, or Backend if workers stopped.
    pub fn submit(&mut self, request: BackendRequest) -> BackendResult<SceneLoadTicket> {
        let requested_ref = request.requested_ref();
        let ticket = self.load_guard.begin(&requested_ref);
        let work = WorkItem {
            ticket: ticket.clone(),
            request,
        };

        match self.request_sender.try_send(work) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                return Err(BackendError::new(
                    BackendErrorKind::Busy,
                    "submit read",
                    anyhow!("the native backend queue is full"),
                ));
            }
            Err(TrySendError::Disconnected(_)) => {
                return Err(BackendError::new(
                    BackendErrorKind::Backend,
                    "submit read",
                    anyhow!("the native backend worker pool stopped"),
                ));
            }
        }

        Ok(ticket)
    }

    /// Poll one completion without blocking the UI thread.
    ///
    /// Superseded payloads are dropped here and never exposed to the caller.
    ///
    /// # Errors
    /// Returns Backend if all workers have stopped and the result queue is empty.
    pub fn try_recv(&self) -> BackendResult<Option<BackendCompletion>> {
        let completion = match self.completion_receiver.try_recv() {
            Ok(completion) => completion,
            Err(TryRecvError::Empty) => return Ok(None),
            Err(TryRecvError::Disconnected) => {
                return Err(BackendError::new(
                    BackendErrorKind::Backend,
                    "receive read",
                    anyhow!("the native backend worker pool stopped"),
                ));
            }
        };

        let status = self.load_guard.settle(&completion.ticket);
        let load = SceneLoadOperation {
            generation: completion.ticket.generation,
            requested_ref: completion.ticket.requested_ref,
            status,
        };
        let outcome = if status == SceneLoadStatus::Current {
            BackendOutcome::Current(completion.result)
        } else {
            BackendOutcome::Superseded
        };
        Ok(Some(BackendCompletion {
            load,
            project_root: completion.project_root,
            outcome,
        }))
    }

    pub fn latest_generation(&self) -> u64 {
        self.load_guard.latest_generation()
    }
}

fn worker_loop(
    receiver: &Mutex<Receiver<WorkItem>>,
    sender: &SyncSender<WorkCompletion>,
    executor: &Executor,
) {
    loop {
        let work = {
            let Ok(receiver) = receiver.lock() else {
                return;
            };
            receiver.recv()
        };
        let Ok(work) = work else {
            return;
        };

        let ticket = work.ticket;
        let project_root = work.request.project_root.clone();
        let result = executor(work.request);
        if sender
            .send(WorkCompletion {
                ticket,
                project_root,
                result,
            })
            .is_err()
        {
            return;
        }
    }
}

fn execute_blocking(request: BackendRequest) -> BackendResult<BackendPayload> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .map_err(|source| {
            BackendError::new(
                BackendErrorKind::Backend,
                "start read runtime",
                AnyError::new(source),
            )
        })?;
    runtime.block_on(execute_read(request))
}

async fn execute_read(request: BackendRequest) -> BackendResult<BackendPayload> {
    validate_operation(&request.operation)?;
    let mut context = read_only_context(&request.project_root)?;
    // Defense in depth: this adapter never retains a config/env/PATH sidecar.
    context.set_sidecar(None);

    match request.operation {
        ReadOperation::Project => read_project(&context)
            .await
            .map(Box::new)
            .map(BackendPayload::Project),
        ReadOperation::Version { branch, version } => read_version(&context, &branch, &version)
            .await
            .map(Box::new)
            .map(BackendPayload::Version),
        ReadOperation::Diff {
            branch,
            from_version,
            to_version,
        } => read_diff(&context, &branch, &from_version, &to_version)
            .await
            .map(Box::new)
            .map(BackendPayload::Diff),
    }
}

fn validate_operation(operation: &ReadOperation) -> BackendResult<()> {
    let identifiers: &[&str] = match operation {
        ReadOperation::Project => &[],
        ReadOperation::Version { branch, version } => &[branch, version],
        ReadOperation::Diff {
            branch,
            from_version,
            to_version,
        } => &[branch, from_version, to_version],
    };
    if identifiers.iter().any(|value| value.trim().is_empty()) {
        return Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "validate read request",
            anyhow!("branch and version identifiers must not be empty"),
        ));
    }
    Ok(())
}

fn read_only_context(project_root: &Path) -> BackendResult<AppContext> {
    AppContext::new_read_only(project_root).map_err(|source| {
        // Repository discovery/config validation stays in ext-api. Only classify
        // typed causes here; constructor prose is never part of the contract.
        let io_error = source
            .chain()
            .find_map(|cause| cause.downcast_ref::<std::io::Error>());
        let mut kind = classify_source(&source);
        if kind == BackendErrorKind::Backend {
            kind = match io_error.map(std::io::Error::kind) {
                Some(std::io::ErrorKind::NotFound) => BackendErrorKind::NotFound,
                Some(_) => BackendErrorKind::Backend,
                None => BackendErrorKind::InvalidProject,
            };
        }
        BackendError::new(kind, "open project", source)
    })
}

async fn read_project(context: &AppContext) -> BackendResult<ProjectRead> {
    let status = project_status(context, StatusOptions { verbose: false })
        .await
        .map_err(|source| {
            BackendError::new(
                BackendErrorKind::InvalidProject,
                "read project status",
                source,
            )
        })?;
    let branches = list_branches(context)
        .await
        .map_err(|source| BackendError::from_backend("list branches", source))?;
    Ok(ProjectRead { status, branches })
}

async fn read_branch(
    context: &AppContext,
    branch: &str,
) -> BackendResult<(ListBranchesResult, ListVersionsResult)> {
    let branches = list_branches(context)
        .await
        .map_err(|source| BackendError::from_backend("list branches", source))?;
    if !branches.branches.iter().any(|item| item.name == branch) {
        return Err(BackendError::new(
            BackendErrorKind::NotFound,
            "select branch",
            anyhow!("the selected branch is absent from the authoritative branch list"),
        ));
    }
    let versions = list_versions(context, Some(branch), false)
        .await
        .map_err(|source| BackendError::from_backend("list versions", source))?;
    Ok((branches, versions))
}

fn require_version(versions: &ListVersionsResult, version: &str) -> BackendResult<()> {
    if versions
        .commits
        .iter()
        .any(|commit| commit.version_id.as_deref() == Some(version))
    {
        return Ok(());
    }
    Err(BackendError::new(
        BackendErrorKind::NotFound,
        "select version",
        anyhow!("the selected version is absent from the authoritative version list"),
    ))
}

async fn validated_detail(
    context: &AppContext,
    branch: &str,
    version: &str,
) -> BackendResult<VersionDetail> {
    let version_ref = format!("{branch}/{version}");
    show_version(context, &version_ref)
        .await
        .map_err(|source| BackendError::from_backend("show version", source))
}

async fn read_version(
    context: &AppContext,
    branch: &str,
    version: &str,
) -> BackendResult<VersionRead> {
    let (branches, versions) = read_branch(context, branch).await?;
    require_version(&versions, version)?;
    let detail = validated_detail(context, branch, version).await?;
    let version_ref = format!("{branch}/{version}");
    let overview = get_project_overview(context, &version_ref)
        .map_err(AnyError::new)
        .map_err(|source| BackendError::from_backend("read project overview", source))?;
    Ok(VersionRead {
        branches,
        versions,
        detail,
        overview,
    })
}

async fn read_diff(
    context: &AppContext,
    branch: &str,
    from_version: &str,
    to_version: &str,
) -> BackendResult<DiffRead> {
    let (branches, versions) = read_branch(context, branch).await?;
    require_version(&versions, from_version)?;
    require_version(&versions, to_version)?;
    // `show_version` performs the source API's path/ref validation before the
    // overview/diff surfaces receive a qualified reference.
    let from_detail = validated_detail(context, branch, from_version).await?;
    let to_detail = validated_detail(context, branch, to_version).await?;
    let from_ref = format!("{branch}/{from_version}");
    let to_ref = format!("{branch}/{to_version}");
    let diff = diff_versions(context, &from_ref, &to_ref)
        .await
        .map_err(|source| BackendError::from_backend("diff versions", source))?;
    Ok(DiffRead {
        branches,
        versions,
        from_detail,
        to_detail,
        diff,
    })
}

#[cfg(test)]
#[path = "native_backend_tests.rs"]
mod tests;
