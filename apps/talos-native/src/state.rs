//! Application state and its only transition function, [`State::apply`].
//!
//! Ownership boundaries:
//! * The **session** owns document identity: which version is requested, the
//!   load generation, and the loaded immutable [`Document`].
//! * Each **workspace** owns its own state ([`ModelState`], [`ResultsState`])
//!   and keeps it across tab switches. Version-scoped parts (the selection,
//!   the results query's story range) are revalidated on every version change.
//! * **Views** (the future GPU viewport, charts) own only presentation state
//!   such as the camera; they read state and return intents, which `apply`
//!   accepts or refuses. They never mutate identity or selection themselves.
//!
//! `apply` performs no I/O: loading is returned as an [`Effect`] for the host
//! to run, and its response comes back as [`Action::Loaded`] carrying the
//! [`Ticket`] it was issued with.
use crate::document::Document;
use crate::project::{Project, VersionEntry, VersionId};
use lab_e2k_mesh::pick::{Scope, Selection};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workspace {
    Model,
    Results,
    Compare,
    History,
    Reports,
}
impl Workspace {
    pub const ALL: [Self; 5] = [
        Self::Model,
        Self::Results,
        Self::Compare,
        Self::History,
        Self::Reports,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Results => "Results",
            Self::Compare => "Compare",
            Self::History => "History",
            Self::Reports => "Reports",
        }
    }
    /// `None` when the workspace works in this build; otherwise what it waits for.
    pub fn unavailable_reason(self) -> Option<&'static str> {
        match self {
            Self::Model | Self::Results => None,
            Self::Compare => Some(
                "Version comparison needs the canonical comparison interface from the \
                 EtabExtension backend, which this prototype does not integrate yet.",
            ),
            Self::History => Some(
                "History needs the authoritative version-control backend (ext-api, APP-01D). \
                 The synthetic demo has no commit history to show.",
            ),
            Self::Reports => Some(
                "Reports will preview and export through ext-report. It is not integrated in \
                 this prototype, and nothing has been analyzed to report on.",
            ),
        }
    }
    fn step(self, forward: bool) -> Self {
        let at = Self::ALL.iter().position(|w| *w == self).unwrap_or(0);
        let n = Self::ALL.len();
        Self::ALL[if forward {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

/// The identity a load was requested with. Only the session's current ticket
/// is accepted; switching versions makes every earlier ticket stale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub version: VersionId,
    pub generation: u64,
}

pub enum DocumentState {
    Loading(Ticket),
    Ready {
        ticket: Ticket,
        document: Arc<Document>,
    },
    Failed {
        ticket: Ticket,
        message: String,
    },
}

pub struct Session {
    pub project: Project,
    pub document: DocumentState,
}
impl Session {
    pub fn ticket(&self) -> &Ticket {
        match &self.document {
            DocumentState::Loading(ticket)
            | DocumentState::Ready { ticket, .. }
            | DocumentState::Failed { ticket, .. } => ticket,
        }
    }
    /// The version the user asked for, whether or not it has loaded.
    pub fn requested(&self) -> &VersionId {
        &self.ticket().version
    }
    pub fn ready(&self) -> Option<&Arc<Document>> {
        match &self.document {
            DocumentState::Ready { document, .. } => Some(document),
            _ => None,
        }
    }
    /// Selection scope: geometry revision plus load generation, so even a reload
    /// of identical bytes as another version expires a selection.
    pub fn scope(&self) -> Option<Scope> {
        match &self.document {
            DocumentState::Ready { ticket, document } => {
                Some((document.mesh().revision(), ticket.generation))
            }
            _ => None,
        }
    }
}

/// How the Model workspace places its Explorer and Inspector.
///
/// Floating cards (the default) give the model the whole canvas; docked
/// panes never cover geometry. Both show the same content; neither is a permanent decision
/// (owner, #52).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelMode {
    Docked,
    Floating,
}

/// Which panel a compact window shows when it can show only one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Explorer,
    Inspector,
}

pub struct ModelState {
    pub panels: PanelMode,
    /// The user's intent for each panel. A compact window shows at most one
    /// of them (see [`ModelState::shows_explorer`]) without changing either,
    /// so widening the window again restores what the user asked for.
    pub explorer_open: bool,
    pub inspector_open: bool,
    /// The window is below the compact width (owner, #54 review): one
    /// contextual panel at a time. Reported by the page, never by a load.
    pub compact: bool,
    /// The panel a compact window prefers: Explorer by default, Inspector
    /// after a member pick, or whichever the user last asked for.
    pub context: Context,
    /// Highlighted story, by name; kept across versions only if it exists there.
    pub story: Option<String>,
    /// Other visible entities hit at the same depth by the last accepted view
    /// pick: the winner is then not decided by depth and must be disclosed.
    pub ambiguous: usize,
    selection: Selection,
}
impl ModelState {
    /// Whether the Explorer is drawn: the user's intent when wide; when
    /// compact, only if it is the context (or the Inspector is closed).
    pub fn shows_explorer(&self) -> bool {
        self.explorer_open
            && (!self.compact || self.context == Context::Explorer || !self.inspector_open)
    }
    /// Whether the Inspector is drawn; compact windows never draw both.
    pub fn shows_inspector(&self) -> bool {
        self.inspector_open
            && (!self.compact || self.context == Context::Inspector || !self.explorer_open)
    }
    /// E / the explorer buttons. Compact and showing the Inspector instead:
    /// swap to the Explorer, keeping both intents. Otherwise a plain toggle.
    fn toggle_explorer(&mut self) {
        if self.compact && self.explorer_open && !self.shows_explorer() {
            self.context = Context::Explorer;
        } else if self.compact && self.shows_explorer() && self.inspector_open {
            self.context = Context::Inspector;
        } else {
            self.explorer_open ^= true;
            if self.explorer_open {
                self.context = Context::Explorer;
            }
        }
    }
    /// I / the inspector buttons, the mirror of [`Self::toggle_explorer`].
    fn toggle_inspector(&mut self) {
        if self.compact && self.inspector_open && !self.shows_inspector() {
            self.context = Context::Inspector;
        } else if self.compact && self.shows_inspector() && self.explorer_open {
            self.context = Context::Explorer;
        } else {
            self.inspector_open ^= true;
            if self.inspector_open {
                self.context = Context::Inspector;
            }
        }
    }
    /// Selected entity indices, only for the scope they were made in.
    pub fn selected(&self, scope: Scope) -> &[usize] {
        self.selection.entities(scope)
    }
}

/// A stored story response (`lab-results`): wind or seismic drift, wind
/// displacement, story shear, or the stored story moment components Mx and My
/// (no overturning axis is inferred).
pub use lab_results::response::Response as Metric;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    X,
    Y,
}
impl Direction {
    pub fn cached(self) -> lab_results::data::Direction {
        match self {
            Self::X => lab_results::data::Direction::X,
            Self::Y => lab_results::data::Direction::Y,
        }
    }
}
/// Inclusive, by story name; `top` is at or above `bottom`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryRange {
    pub top: String,
    pub bottom: String,
}
/// What the Results workspace asks for. It is a request, not data: no value is
/// shown unless an analyzed result set for exactly `version` answers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultsQuery {
    /// The version this query was last validated against.
    pub version: Option<VersionId>,
    pub metric: Metric,
    pub direction: Direction,
    /// A stored group (drift and displacement envelopes only); only ever one
    /// the version's cached results hold.
    pub group: Option<String>,
    /// A stored load case or combination; none without cached results.
    pub case: Option<String>,
    /// `None` is every story in the version.
    pub stories: Option<StoryRange>,
}
/// The Results page's content for the current query.
#[derive(Debug, PartialEq)]
pub enum Shown {
    /// The stored profile, cut to the requested story range.
    Plot(lab_results::data::Plot),
    /// The requested range names stories (these) the cached profile lacks.
    RangeMismatch(Vec<String>),
    /// No cached results, or nothing stored for this query.
    Unavailable,
}

pub struct ResultsState {
    pub query: ResultsQuery,
    /// Why the query changed without the user asking, e.g. a story range that
    /// does not exist in the newly selected version.
    pub notice: Option<String>,
    /// The story highlighted in the chart and the table; per version.
    pub story: Option<String>,
}

pub enum Action {
    SelectWorkspace(Workspace),
    NextWorkspace,
    PreviousWorkspace,
    SwitchVersion(VersionId),
    Loaded {
        ticket: Ticket,
        result: Result<Arc<Document>, String>,
    },
    ToggleExplorer,
    ToggleInspector,
    /// The page's width class changed (below or above the compact width).
    SetCompact(bool),
    /// Move the Explorer's story filter to the selection's story (or clear it
    /// when the selection spans stories), keeping the selection.
    RevealSelection,
    /// From a view or the explorer, made in `scope`; refused in any other.
    Pick {
        scope: Scope,
        entity: Option<usize>,
        toggle: bool,
        /// Other entities at the same depth (0 for explorer picks).
        coincident: usize,
    },
    SelectStory(Option<String>),
    SetPanelMode(PanelMode),
    SetMetric(Metric),
    SetDirection(Direction),
    SetGroup(String),
    SetCase(String),
    /// Highlight one story of the shown profile in the chart and the table.
    SelectResultStory(Option<String>),
    SetStoryRange(Option<StoryRange>),
}

/// Work for the host. `apply` never performs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Load { ticket: Ticket, entry: VersionEntry },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    UnknownVersion,
    /// A load response for a ticket that is no longer current.
    StaleResponse,
    /// A response whose document is not the version its ticket asked for.
    MismatchedResponse,
    /// A pick made against another version, generation or geometry.
    StaleSelection,
    UnknownEntity,
    UnknownStory,
    /// A metric, group or case the version's cached results do not hold.
    NotInResults,
    NoDocument,
}

pub struct State {
    pub workspace: Workspace,
    pub session: Session,
    pub model: ModelState,
    pub results: ResultsState,
    /// Load responses discarded as stale; diagnostic only.
    pub stale_discarded: u64,
    generation: u64,
}

impl State {
    /// Starts on the Model workspace, loading the project's latest version.
    pub fn new(project: Project) -> (Self, Vec<Effect>) {
        let first = project.latest().cloned();
        let ticket = Ticket {
            version: first
                .as_ref()
                .map_or_else(|| VersionId::new("", ""), |entry| entry.id.clone()),
            generation: 1,
        };
        let document = if first.is_some() {
            DocumentState::Loading(ticket.clone())
        } else {
            DocumentState::Failed {
                ticket: ticket.clone(),
                message: "The project has no versions.".to_owned(),
            }
        };
        let state = Self {
            workspace: Workspace::Model,
            session: Session { project, document },
            model: ModelState {
                panels: PanelMode::Floating,
                explorer_open: true,
                inspector_open: true,
                compact: false,
                context: Context::Explorer,
                story: None,
                ambiguous: 0,
                selection: Selection::default(),
            },
            results: ResultsState {
                query: ResultsQuery {
                    version: None,
                    metric: Metric::WindDrift,
                    direction: Direction::X,
                    group: None,
                    case: None,
                    stories: None,
                },
                notice: None,
                story: None,
            },
            stale_discarded: 0,
            generation: 1,
        };
        let effects = first
            .map(|entry| Effect::Load { ticket, entry })
            .into_iter()
            .collect();
        (state, effects)
    }

    /// The single state transition. A refusal leaves the state unchanged,
    /// except that a stale response is counted and a response for the wrong
    /// version fails the load closed instead of leaving it pending.
    ///
    /// # Errors
    /// Why the action was refused; never a panic.
    pub fn apply(&mut self, action: Action) -> Result<Vec<Effect>, Refusal> {
        match action {
            Action::SelectWorkspace(workspace) => self.workspace = workspace,
            Action::NextWorkspace => self.workspace = self.workspace.step(true),
            Action::PreviousWorkspace => self.workspace = self.workspace.step(false),
            Action::SwitchVersion(version) => return self.switch_version(&version),
            Action::Loaded { ticket, result } => self.loaded(&ticket, result)?,
            Action::ToggleExplorer => self.model.toggle_explorer(),
            Action::ToggleInspector => self.model.toggle_inspector(),
            Action::SetCompact(compact) => self.model.compact = compact,
            Action::RevealSelection => self.reveal_selection()?,
            Action::Pick {
                scope,
                entity,
                toggle,
                coincident,
            } => self.pick(scope, entity, toggle, coincident)?,
            Action::SelectStory(story) => self.select_story(story)?,
            Action::SetPanelMode(mode) => self.model.panels = mode,
            Action::SetMetric(metric) => self.set_metric(metric)?,
            Action::SetDirection(direction) => {
                self.results.query.direction = direction;
                let snapshot = self.cached_arc();
                self.reconcile_results(snapshot.as_ref());
            }
            Action::SetGroup(group) => self.set_choice(Some(group), None)?,
            Action::SetCase(case) => self.set_choice(None, Some(case))?,
            Action::SelectResultStory(story) => self.select_result_story(story)?,
            Action::SetStoryRange(range) => self.set_story_range(range)?,
        }
        Ok(Vec::new())
    }

    fn switch_version(&mut self, version: &VersionId) -> Result<Vec<Effect>, Refusal> {
        let entry = self
            .session
            .project
            .version(version)
            .cloned()
            .ok_or(Refusal::UnknownVersion)?;
        if self.session.requested() == version
            && !matches!(self.session.document, DocumentState::Failed { .. })
        {
            return Ok(Vec::new());
        }
        self.generation += 1;
        let ticket = Ticket {
            version: version.clone(),
            generation: self.generation,
        };
        self.session.document = DocumentState::Loading(ticket.clone());
        self.model.selection = Selection::default();
        self.model.context = Context::Explorer;
        self.model.ambiguous = 0;
        // A highlighted result story belongs to the version it was picked in.
        self.results.story = None;
        Ok(vec![Effect::Load { ticket, entry }])
    }

    fn loaded(
        &mut self,
        ticket: &Ticket,
        result: Result<Arc<Document>, String>,
    ) -> Result<(), Refusal> {
        if !matches!(&self.session.document, DocumentState::Loading(current) if current == ticket) {
            self.stale_discarded += 1;
            return Err(Refusal::StaleResponse);
        }
        if let Ok(document) = &result
            && document.version() != &ticket.version
        {
            self.session.document = DocumentState::Failed {
                ticket: ticket.clone(),
                message: format!(
                    "The loader returned {} for a {} request; nothing was shown.",
                    document.version(),
                    ticket.version
                ),
            };
            return Err(Refusal::MismatchedResponse);
        }
        self.session.document = match result {
            Ok(document) => {
                self.revalidate_results(&document);
                if self
                    .model
                    .story
                    .as_deref()
                    .is_some_and(|story| !document.story_names().any(|name| name == story))
                {
                    self.model.story = None;
                }
                DocumentState::Ready {
                    ticket: ticket.clone(),
                    document,
                }
            }
            Err(message) => DocumentState::Failed {
                ticket: ticket.clone(),
                message,
            },
        };
        Ok(())
    }

    fn pick(
        &mut self,
        scope: Scope,
        entity: Option<usize>,
        toggle: bool,
        coincident: usize,
    ) -> Result<(), Refusal> {
        let document = self.session.ready().ok_or(Refusal::NoDocument)?;
        if self.session.scope() != Some(scope) {
            return Err(Refusal::StaleSelection);
        }
        if entity.is_some_and(|index| index >= document.mesh().entities().len()) {
            return Err(Refusal::UnknownEntity);
        }
        self.model.selection.click(scope, entity, toggle);
        self.model.ambiguous = if entity.is_some() { coincident } else { 0 };
        // A member pick makes the Inspector the compact context; clearing the
        // selection hands it back to the Explorer.
        self.model.context = if self.model.selection.entities(scope).is_empty() {
            Context::Explorer
        } else {
            Context::Inspector
        };
        Ok(())
    }

    /// The Explorer filter follows the selection instead of the selection
    /// being dropped: the one case where the story changes and a pick stays,
    /// because afterwards the two agree (owner, #53 Windows note).
    fn reveal_selection(&mut self) -> Result<(), Refusal> {
        let document = self.session.ready().ok_or(Refusal::NoDocument)?;
        let scope = self.session.scope().ok_or(Refusal::NoDocument)?;
        let entities = document.mesh().entities();
        let mut stories = self
            .model
            .selection
            .entities(scope)
            .iter()
            .filter_map(|&index| entities.get(index).map(|e| e.story.as_str()));
        let Some(first) = stories.next() else {
            return Ok(());
        };
        let single = stories.all(|story| story == first);
        self.model.story = single.then(|| first.to_owned());
        Ok(())
    }

    fn select_story(&mut self, story: Option<String>) -> Result<(), Refusal> {
        let document = self.session.ready().ok_or(Refusal::NoDocument)?;
        if story
            .as_deref()
            .is_some_and(|story| !document.story_names().any(|name| name == story))
        {
            return Err(Refusal::UnknownStory);
        }
        // Choosing (or clearing) a story is a new question: a member picked
        // under the previous one, and its ambiguity note, no longer apply.
        if self.model.story != story {
            self.model.selection = Selection::default();
            self.model.ambiguous = 0;
            self.model.context = Context::Explorer;
        }
        self.model.story = story;
        Ok(())
    }

    fn set_story_range(&mut self, range: Option<StoryRange>) -> Result<(), Refusal> {
        let document = self.session.ready().ok_or(Refusal::NoDocument)?;
        if let Some(range) = &range
            && !valid_range(document, range)
        {
            return Err(Refusal::UnknownStory);
        }
        self.results.query.stories = range;
        self.results.notice = None;
        // A highlighted story the new range no longer shows is cleared, so the
        // table and chart never point at a row that is not on screen.
        let off_screen = self.results.story.as_ref().is_some_and(|story| {
            !matches!(self.shown_profile(), Shown::Plot(plot) if plot.stories.contains(story))
        });
        if off_screen {
            self.results.story = None;
        }
        Ok(())
    }

    fn revalidate_results(&mut self, document: &Document) {
        // A notice describes one load; a later load starts without it.
        self.results.notice = None;
        self.results.story = None;
        self.results.query.version = Some(document.version().clone());
        self.reconcile_results(cached_of(document));
        let query = &mut self.results.query;
        if let Some(range) = &query.stories
            && !valid_range(document, range)
        {
            self.results.notice = Some(format!(
                "Story range {}–{} does not exist in {}; showing all stories.",
                range.top,
                range.bottom,
                document.version()
            ));
            query.stories = None;
        }
    }
}

impl State {
    /// The current version's cached results, if loaded and accepted.
    pub fn cached(&self) -> Option<&lab_results::data::Snapshot> {
        let document = self.session.ready()?;
        cached_of(document).map(|s| &**s)
    }
    fn cached_arc(&self) -> Option<Arc<lab_results::data::Snapshot>> {
        let document = self.session.ready()?;
        cached_of(document).cloned()
    }

    /// The stored profile the query asks for, or why there is none.
    ///
    /// # Errors
    /// No cached results, or the query names nothing they hold.
    pub fn profile(&self) -> Result<lab_results::data::Plot, String> {
        profile_of(
            self.cached().ok_or("No cached results")?,
            &self.results.query,
        )
    }

    /// What the Results page shows for the query: the stored profile cut to
    /// the story range, or why it cannot. A range the cached profile does not
    /// name is a mismatch, never silently widened to every story.
    pub fn shown_profile(&self) -> Shown {
        let Ok(mut plot) = self.profile() else {
            return Shown::Unavailable;
        };
        let Some(range) = &self.results.query.stories else {
            return Shown::Plot(plot);
        };
        let at = |name: &str| plot.stories.iter().position(|s| s == name);
        match (at(&range.top), at(&range.bottom)) {
            (Some(a), Some(b)) => {
                let (from, to) = (a.min(b), a.max(b));
                plot.stories = plot.stories[from..=to].to_vec();
                for series in &mut plot.series {
                    series.values = series.values[from..=to].to_vec();
                }
                Shown::Plot(plot)
            }
            (top, bottom) => Shown::RangeMismatch(
                [(top, &range.top), (bottom, &range.bottom)]
                    .into_iter()
                    .filter(|(found, _)| found.is_none())
                    .map(|(_, name)| name.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            ),
        }
    }

    fn set_metric(&mut self, metric: Metric) -> Result<(), Refusal> {
        if let Some(snapshot) = self.cached()
            && !snapshot.responses().contains(&metric)
        {
            return Err(Refusal::NotInResults);
        }
        self.results.query.metric = metric;
        let snapshot = self.cached_arc();
        self.reconcile_results(snapshot.as_ref());
        Ok(())
    }

    /// A group or a case, only ever one the cached results pair with the
    /// rest of the query; a new group keeps the case only if it has rows.
    fn set_choice(&mut self, group: Option<String>, case: Option<String>) -> Result<(), Refusal> {
        let snapshot = self.cached_arc().ok_or(Refusal::NotInResults)?;
        let query = &self.results.query;
        let group_for_cases = group.as_deref().or(query.group.as_deref());
        let choices = snapshot
            .response_choices(query.metric, query.direction.cached(), group_for_cases)
            .map_err(|_absent| Refusal::NotInResults)?;
        if group.as_ref().is_some_and(|g| !choices.groups.contains(g))
            || case.as_ref().is_some_and(|c| !choices.cases.contains(c))
        {
            return Err(Refusal::NotInResults);
        }
        let query = &mut self.results.query;
        if group.is_some() {
            query.group = group;
        }
        if case.is_some() {
            query.case = case;
        }
        self.reconcile_results(Some(&snapshot));
        Ok(())
    }

    fn select_result_story(&mut self, story: Option<String>) -> Result<(), Refusal> {
        if let Some(name) = &story {
            let plot = self.profile().map_err(|_absent| Refusal::NotInResults)?;
            if !plot.stories.contains(name) {
                return Err(Refusal::UnknownStory);
            }
        }
        self.results.story = story;
        Ok(())
    }

    /// Keeps the metric, group and case to what the current version's cached
    /// results hold. A choice that is gone becomes the first stored one, with
    /// a notice; nothing is chosen when there are no cached results.
    fn reconcile_results(&mut self, snapshot: Option<&Arc<lab_results::data::Snapshot>>) {
        let Some(snapshot) = snapshot else {
            let query = &mut self.results.query;
            query.group = None;
            query.case = None;
            return;
        };
        let available = snapshot.responses();
        let query = &self.results.query;
        let mut notices = Vec::new();
        let metric = if available.contains(&query.metric) {
            query.metric
        } else if let Some(&first) = available.first() {
            notices.push(format!(
                "{} is not in these cached results.",
                query.metric.label()
            ));
            first
        } else {
            query.metric
        };
        let direction = query.direction.cached();
        let groups = snapshot
            .response_choices(metric, direction, None)
            .unwrap_or_default()
            .groups;
        let keep = |current: &Option<String>,
                    options: &[String],
                    what: &str,
                    notices: &mut Vec<String>| {
            match current {
                Some(value) if options.contains(value) => Some(value.clone()),
                previous => {
                    if let Some(value) = previous {
                        notices.push(format!("{what} {value} is not stored here."));
                    }
                    options.first().cloned()
                }
            }
        };
        let group = if metric.grouped() {
            keep(&query.group, &groups, "Group", &mut notices)
        } else {
            None
        };
        // Only cases with rows for that group, so the pair always plots.
        let cases = snapshot
            .response_choices(metric, direction, group.as_deref())
            .unwrap_or_default()
            .cases;
        let case = keep(&query.case, &cases, "Case", &mut notices);
        let story_kept = self.results.story.clone();
        let query = &mut self.results.query;
        query.metric = metric;
        query.group = group;
        query.case = case;
        if !notices.is_empty() {
            self.results.notice = Some(notices.join(" "));
        }
        // The highlighted story stays only while the shown profile has it.
        if let Some(story) = story_kept
            && !profile_of(snapshot, &self.results.query)
                .is_ok_and(|plot| plot.stories.contains(&story))
        {
            self.results.story = None;
        }
    }
}

fn cached_of(document: &Document) -> Option<&Arc<lab_results::data::Snapshot>> {
    match document.results() {
        crate::document::Results::Loaded(snapshot) => Some(snapshot),
        _ => None,
    }
}

fn profile_of(
    snapshot: &lab_results::data::Snapshot,
    query: &ResultsQuery,
) -> Result<lab_results::data::Plot, String> {
    let case = query.case.as_deref().ok_or("No stored case")?;
    snapshot.response(
        query.metric,
        query.direction.cached(),
        query.group.as_deref(),
        case,
    )
}

fn valid_range(document: &Document, range: &StoryRange) -> bool {
    let elevation = |name: &str| {
        document
            .mesh()
            .stories()
            .iter()
            .find(|story| story.name == name)
            .map(|story| story.elevation_m)
    };
    matches!(
        (elevation(&range.top), elevation(&range.bottom)),
        (Some(top), Some(bottom)) if top >= bottom
    )
}
