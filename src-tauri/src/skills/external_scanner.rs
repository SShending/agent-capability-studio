use super::{candidate::CandidateAuditSnapshot, Finding};
use reqwest::Url;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path},
    sync::Arc,
};
use thiserror::Error;

const MAX_FINDINGS: usize = 500;
const MAX_TEXT: usize = 4_000;
const MAX_LINE_SPAN: usize = 13;
const MIN_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 10 * 60 * 1_000;

#[derive(Debug, Error)]
pub enum ExternalScannerError {
    #[error("The external scanner identity or configuration is invalid.")]
    InvalidScannerIdentity,
    #[error("The external scanner plan is invalid for this Candidate Skill revision.")]
    InvalidPlan,
    #[error("The scanner plan changed. Review the scanner and files again.")]
    StalePlan,
    #[error("The external scanner could not complete the requested scan.")]
    AdapterFailure,
    #[error("The external scanner report is malformed or incomplete.")]
    InvalidReport,
    #[error("The external scanner reported evidence that is not grounded in the scanned files.")]
    UngroundedEvidence,
    #[error("The external scanner report exceeded the accepted limits.")]
    OutputLimit,
    #[error("The external scanner timed out.")]
    Timeout,
    #[error("The external scanner was cancelled.")]
    Cancelled,
}

impl ExternalScannerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidScannerIdentity => "INVALID_SCANNER_IDENTITY",
            Self::InvalidPlan => "INVALID_SCANNER_PLAN",
            Self::StalePlan => "STALE_SCANNER_PLAN",
            Self::AdapterFailure => "SCANNER_ADAPTER_FAILURE",
            Self::InvalidReport => "INVALID_SCANNER_REPORT",
            Self::UngroundedEvidence => "UNGROUNDED_SCANNER_EVIDENCE",
            Self::OutputLimit => "SCANNER_OUTPUT_LIMIT",
            Self::Timeout => "SCANNER_TIMEOUT",
            Self::Cancelled => "SCANNER_CANCELLED",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerIdentity {
    pub adapter_id: String,
    pub display_name: String,
    pub adapter_version: String,
    pub product_version: String,
    pub ruleset_version: String,
    pub execution_sha256: String,
    pub configuration_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScannerDataHandling {
    LocalOnly,
    External { endpoint: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScannerExecutionKind {
    Embedded,
    LocalProcess,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerFileEvidence {
    pub path: String,
    pub size: usize,
    pub sha256: String,
    pub executable: bool,
}

#[derive(Clone, Debug)]
pub struct ScannerCandidateManifest {
    pub candidate_hash: String,
    pub files: Vec<ScannerFileEvidence>,
}

#[derive(Clone, Debug)]
pub struct ScannerInputFile {
    pub evidence: ScannerFileEvidence,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ScannerInput {
    pub candidate_hash: String,
    pub files: Vec<ScannerInputFile>,
}

#[derive(Clone, Debug)]
pub struct ScannerAdapterPlan {
    pub identity: ScannerIdentity,
    pub execution: ScannerExecutionKind,
    pub data_handling: ScannerDataHandling,
    pub configuration_summary: String,
    pub file_paths: Vec<String>,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerPlan {
    pub identity: ScannerIdentity,
    pub execution: ScannerExecutionKind,
    pub data_handling: ScannerDataHandling,
    pub configuration_summary: String,
    pub candidate_hash: String,
    pub files: Vec<ScannerFileEvidence>,
    pub timeout_ms: u64,
    pub plan_hash: String,
}

#[derive(Clone, Copy, Debug)]
pub enum ScannerSeverity {
    Blocker,
    Warning,
    Info,
}

#[derive(Clone, Copy, Debug)]
pub enum ScannerConfidence {
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug)]
pub enum ScannerFindingLocation {
    Lines {
        path: String,
        start: usize,
        end: usize,
    },
    File {
        path: String,
    },
}

#[derive(Clone, Debug)]
pub struct ReportedScannerFinding {
    pub id: String,
    pub severity: ScannerSeverity,
    pub confidence: ScannerConfidence,
    pub title: String,
    pub explanation: String,
    pub location: ScannerFindingLocation,
}

#[derive(Clone, Debug)]
pub struct ReportedScannerResult {
    pub identity: ScannerIdentity,
    pub candidate_hash: String,
    pub scanned_files: Vec<ScannerFileEvidence>,
    pub findings: Vec<ReportedScannerFinding>,
}

#[derive(Clone, Debug)]
pub struct ScannerRawOutput {
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerAuditContribution {
    pub candidate_hash: String,
    pub scanner: ScannerIdentity,
    pub execution: ScannerExecutionKind,
    pub data_handling: ScannerDataHandling,
    pub configuration_summary: String,
    pub scanned_files: Vec<ScannerFileEvidence>,
    pub findings: Vec<Finding>,
    pub verdict: String,
    pub plan_hash: String,
    pub raw_result_sha256: String,
}

pub trait ExternalScannerAdapter: Send + Sync {
    // Planning must not launch a process, contact a network, or inspect file bytes.
    fn plan(
        &self,
        candidate: &ScannerCandidateManifest,
    ) -> Result<ScannerAdapterPlan, ExternalScannerError>;

    // Parsing is vendor-specific, but must not launch a process or contact a network.
    fn parse(
        &self,
        output: &ScannerRawOutput,
        input: &ScannerInput,
        plan: &ScannerPlan,
    ) -> Result<ReportedScannerResult, ExternalScannerError>;
}

pub trait ScannerRuntime: Send + Sync {
    // The runtime owns contained materialization, fixed adapter dispatch, timeout,
    // cancellation, output bounds, and cleanup. It never executes input files.
    fn execute(
        &self,
        input: &ScannerInput,
        plan: &ScannerPlan,
        cancellation: &dyn ScannerCancellation,
    ) -> Result<ScannerRawOutput, ExternalScannerError>;
}

pub trait ScannerCancellation: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

pub struct ExternalScannerManager {
    runtime: Arc<dyn ScannerRuntime>,
}

impl ExternalScannerManager {
    pub fn new(runtime: Arc<dyn ScannerRuntime>) -> Self {
        Self { runtime }
    }

    pub fn preview(
        &self,
        adapter: &dyn ExternalScannerAdapter,
        snapshot: &CandidateAuditSnapshot,
    ) -> Result<ScannerPlan, ExternalScannerError> {
        let input = scanner_input(snapshot)?;
        self.preview_input(adapter, &input)
    }

    fn preview_input(
        &self,
        adapter: &dyn ExternalScannerAdapter,
        input: &ScannerInput,
    ) -> Result<ScannerPlan, ExternalScannerError> {
        let manifest = ScannerCandidateManifest {
            candidate_hash: input.candidate_hash.clone(),
            files: input
                .files
                .iter()
                .map(|file| file.evidence.clone())
                .collect(),
        };
        let requested = adapter.plan(&manifest)?;
        validate_identity(&requested.identity)?;
        validate_data_handling(&requested.data_handling)?;
        if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&requested.timeout_ms)
            || !valid_display_text(&requested.configuration_summary, 500)
        {
            return Err(ExternalScannerError::InvalidPlan);
        }

        let by_path: HashMap<_, _> = manifest
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect();
        let unique: HashSet<_> = requested.file_paths.iter().collect();
        if requested.file_paths.is_empty()
            || unique.len() != requested.file_paths.len()
            || !unique.iter().any(|path| path.as_str() == "SKILL.md")
        {
            return Err(ExternalScannerError::InvalidPlan);
        }
        let mut files = requested
            .file_paths
            .iter()
            .map(|path| {
                by_path
                    .get(path.as_str())
                    .cloned()
                    .cloned()
                    .ok_or(ExternalScannerError::InvalidPlan)
            })
            .collect::<Result<Vec<_>, _>>()?;
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let plan_hash = plan_hash(
            &requested.identity,
            requested.execution,
            &requested.data_handling,
            &requested.configuration_summary,
            &manifest.candidate_hash,
            &files,
            requested.timeout_ms,
        );
        Ok(ScannerPlan {
            identity: requested.identity,
            execution: requested.execution,
            data_handling: requested.data_handling,
            configuration_summary: requested.configuration_summary,
            candidate_hash: manifest.candidate_hash,
            files,
            timeout_ms: requested.timeout_ms,
            plan_hash,
        })
    }

    pub fn run(
        &self,
        adapter: &dyn ExternalScannerAdapter,
        snapshot: &CandidateAuditSnapshot,
        expected_plan_hash: &str,
        cancellation: &dyn ScannerCancellation,
    ) -> Result<ScannerAuditContribution, ExternalScannerError> {
        let mut input = scanner_input(snapshot)?;
        let plan = self.preview_input(adapter, &input)?;
        if expected_plan_hash.is_empty() || expected_plan_hash != plan.plan_hash {
            return Err(ExternalScannerError::StalePlan);
        }
        if cancellation.is_cancelled() {
            return Err(ExternalScannerError::Cancelled);
        }
        let planned: HashSet<_> = plan.files.iter().map(|file| file.path.as_str()).collect();
        input
            .files
            .retain(|file| planned.contains(file.evidence.path.as_str()));
        let output = self.runtime.execute(&input, &plan, cancellation)?;
        if cancellation.is_cancelled() {
            return Err(ExternalScannerError::Cancelled);
        }
        let raw_result_sha256 = sha256(&output.bytes);
        let report = adapter.parse(&output, &input, &plan)?;
        normalize_report(report, plan, &input, raw_result_sha256)
    }
}

fn scanner_input(snapshot: &CandidateAuditSnapshot) -> Result<ScannerInput, ExternalScannerError> {
    let mut paths = HashSet::new();
    for file in &snapshot.files {
        if file.manifest.size != file.bytes.len()
            || file.manifest.sha256 != sha256(&file.bytes)
            || !paths.insert(file.manifest.path.as_str())
            || !valid_relative_path(&file.manifest.path)
        {
            return Err(ExternalScannerError::InvalidPlan);
        }
    }
    let fingerprint = snapshot
        .files
        .iter()
        .map(|file| {
            format!(
                "{}:{}:{}:{}",
                file.manifest.path,
                file.manifest.size,
                file.manifest.sha256,
                file.manifest.executable
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    if snapshot.candidate_hash != sha256(fingerprint.as_bytes()) {
        return Err(ExternalScannerError::InvalidPlan);
    }
    Ok(ScannerInput {
        candidate_hash: snapshot.candidate_hash.clone(),
        files: snapshot
            .files
            .iter()
            .map(|file| ScannerInputFile {
                evidence: ScannerFileEvidence {
                    path: file.manifest.path.clone(),
                    size: file.manifest.size,
                    sha256: file.manifest.sha256.clone(),
                    executable: file.manifest.executable,
                },
                bytes: file.bytes.clone(),
            })
            .collect(),
    })
}

fn normalize_report(
    report: ReportedScannerResult,
    plan: ScannerPlan,
    input: &ScannerInput,
    raw_result_sha256: String,
) -> Result<ScannerAuditContribution, ExternalScannerError> {
    if report.identity != plan.identity || report.candidate_hash != plan.candidate_hash {
        return Err(ExternalScannerError::InvalidReport);
    }
    let mut scanned = report.scanned_files.clone();
    scanned.sort_by(|left, right| left.path.cmp(&right.path));
    if scanned != plan.files {
        return Err(ExternalScannerError::InvalidReport);
    }
    if report.findings.len() > MAX_FINDINGS {
        return Err(ExternalScannerError::OutputLimit);
    }
    let by_path: HashMap<_, _> = input
        .files
        .iter()
        .map(|file| (file.evidence.path.as_str(), file))
        .collect();
    let mut ids = HashSet::new();
    let findings = report
        .findings
        .into_iter()
        .map(|finding| {
            if !valid_finding_id(&finding.id)
                || !ids.insert(finding.id.clone())
                || finding.title.trim().is_empty()
                || finding.title.len() > 300
                || finding.explanation.trim().is_empty()
                || finding.explanation.len() > MAX_TEXT
            {
                return Err(ExternalScannerError::InvalidReport);
            }
            let (path, line_start, line_end, evidence) = match finding.location {
                ScannerFindingLocation::Lines { path, start, end } => {
                    let file = by_path
                        .get(path.as_str())
                        .ok_or(ExternalScannerError::UngroundedEvidence)?;
                    let text = std::str::from_utf8(&file.bytes)
                        .map_err(|_| ExternalScannerError::UngroundedEvidence)?;
                    let lines = text.lines().collect::<Vec<_>>();
                    if start == 0
                        || end < start
                        || end > lines.len()
                        || end - start + 1 > MAX_LINE_SPAN
                    {
                        return Err(ExternalScannerError::UngroundedEvidence);
                    }
                    (
                        path,
                        Some(start),
                        Some(end),
                        lines[start - 1..end].join("\n"),
                    )
                }
                ScannerFindingLocation::File { path } => {
                    let file = by_path
                        .get(path.as_str())
                        .ok_or(ExternalScannerError::UngroundedEvidence)?;
                    (
                        path,
                        None,
                        None,
                        format!(
                            "{} · {} bytes · SHA-256 {}",
                            file.evidence.path, file.evidence.size, file.evidence.sha256
                        ),
                    )
                }
            };
            if evidence.len() > MAX_TEXT {
                return Err(ExternalScannerError::OutputLimit);
            }
            Ok(Finding {
                id: format!("scanner:{}:{}", plan.identity.adapter_id, finding.id),
                severity: severity_label(finding.severity).into(),
                title: finding.title,
                explanation: finding.explanation,
                evidence,
                confidence: confidence_label(finding.confidence).into(),
                source: format!("scanner:{}", plan.identity.adapter_id),
                file_path: Some(path),
                line_start,
                line_end,
                disposition: "pending-review".into(),
                review_note: None,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let verdict = if findings.iter().any(|finding| finding.severity == "blocker") {
        "block"
    } else if findings.iter().any(|finding| finding.severity == "warning") {
        "review"
    } else {
        "clear"
    };
    Ok(ScannerAuditContribution {
        candidate_hash: plan.candidate_hash,
        scanner: plan.identity,
        execution: plan.execution,
        data_handling: plan.data_handling,
        configuration_summary: plan.configuration_summary,
        scanned_files: scanned,
        findings,
        verdict: verdict.into(),
        plan_hash: plan.plan_hash,
        raw_result_sha256,
    })
}

fn validate_identity(identity: &ScannerIdentity) -> Result<(), ExternalScannerError> {
    if !valid_finding_id(&identity.adapter_id)
        || !valid_display_text(&identity.display_name, 200)
        || !valid_version(&identity.adapter_version)
        || !valid_version(&identity.product_version)
        || !valid_version(&identity.ruleset_version)
        || !valid_sha256(&identity.execution_sha256)
        || !valid_sha256(&identity.configuration_sha256)
    {
        Err(ExternalScannerError::InvalidScannerIdentity)
    } else {
        Ok(())
    }
}

fn validate_data_handling(handling: &ScannerDataHandling) -> Result<(), ExternalScannerError> {
    match handling {
        ScannerDataHandling::LocalOnly => Ok(()),
        ScannerDataHandling::External { endpoint } => {
            let url = Url::parse(endpoint).map_err(|_| ExternalScannerError::InvalidPlan)?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || url.username() != ""
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                Err(ExternalScannerError::InvalidPlan)
            } else {
                Ok(())
            }
        }
    }
}

fn plan_hash(
    identity: &ScannerIdentity,
    execution: ScannerExecutionKind,
    handling: &ScannerDataHandling,
    configuration_summary: &str,
    candidate_hash: &str,
    files: &[ScannerFileEvidence],
    timeout_ms: u64,
) -> String {
    let bytes = serde_json::to_vec(&(
        identity,
        execution,
        handling,
        configuration_summary,
        candidate_hash,
        files,
        timeout_ms,
    ))
    .expect("serializable scanner plan");
    sha256(&bytes)
}

fn severity_label(value: ScannerSeverity) -> &'static str {
    match value {
        ScannerSeverity::Blocker => "blocker",
        ScannerSeverity::Warning => "warning",
        ScannerSeverity::Info => "info",
    }
}

fn confidence_label(value: ScannerConfidence) -> &'static str {
    match value {
        ScannerConfidence::High => "high",
        ScannerConfidence::Medium => "medium",
        ScannerConfidence::Low => "low",
    }
}

fn valid_version(value: &str) -> bool {
    valid_display_text(value, 200)
}

fn valid_display_text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

fn valid_finding_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !value.contains(['\\', '\0'])
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::candidate::{CandidateAuditSnapshotFile, CandidateFile, CandidateStager};
    use std::{
        fs,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
    };
    use tempfile::TempDir;

    struct FakeAdapter {
        identity: Mutex<ScannerIdentity>,
        configuration_summary: Mutex<String>,
        findings: Mutex<Vec<ReportedScannerFinding>>,
        parse_calls: AtomicUsize,
        handling: ScannerDataHandling,
        omit_last_file: bool,
        flip_executable: bool,
    }

    struct FakeRuntime {
        execute_calls: AtomicUsize,
        output: Vec<u8>,
    }

    struct StaticCancellation(bool);

    impl ScannerCancellation for StaticCancellation {
        fn is_cancelled(&self) -> bool {
            self.0
        }
    }

    impl ExternalScannerAdapter for FakeAdapter {
        fn plan(
            &self,
            candidate: &ScannerCandidateManifest,
        ) -> Result<ScannerAdapterPlan, ExternalScannerError> {
            Ok(ScannerAdapterPlan {
                identity: self.identity.lock().unwrap().clone(),
                execution: ScannerExecutionKind::LocalProcess,
                data_handling: self.handling.clone(),
                configuration_summary: self.configuration_summary.lock().unwrap().clone(),
                file_paths: candidate
                    .files
                    .iter()
                    .map(|file| file.path.clone())
                    .collect(),
                timeout_ms: 30_000,
            })
        }

        fn parse(
            &self,
            output: &ScannerRawOutput,
            input: &ScannerInput,
            _plan: &ScannerPlan,
        ) -> Result<ReportedScannerResult, ExternalScannerError> {
            self.parse_calls.fetch_add(1, Ordering::Relaxed);
            if output.bytes != b"fixture scanner output" {
                return Err(ExternalScannerError::InvalidReport);
            }
            let mut scanned_files = input
                .files
                .iter()
                .map(|file| file.evidence.clone())
                .collect::<Vec<_>>();
            if self.omit_last_file {
                scanned_files.pop();
            }
            if self.flip_executable {
                scanned_files[0].executable = !scanned_files[0].executable;
            }
            Ok(ReportedScannerResult {
                identity: self.identity.lock().unwrap().clone(),
                candidate_hash: input.candidate_hash.clone(),
                scanned_files,
                findings: self.findings.lock().unwrap().clone(),
            })
        }
    }

    impl ScannerRuntime for FakeRuntime {
        fn execute(
            &self,
            _input: &ScannerInput,
            _plan: &ScannerPlan,
            cancellation: &dyn ScannerCancellation,
        ) -> Result<ScannerRawOutput, ExternalScannerError> {
            self.execute_calls.fetch_add(1, Ordering::Relaxed);
            if cancellation.is_cancelled() {
                return Err(ExternalScannerError::Cancelled);
            }
            Ok(ScannerRawOutput {
                bytes: self.output.clone(),
            })
        }
    }

    fn identity() -> ScannerIdentity {
        ScannerIdentity {
            adapter_id: "fixture-static".into(),
            display_name: "Fixture Static Scanner".into(),
            adapter_version: "1".into(),
            product_version: "2.0.0".into(),
            ruleset_version: "2026-08".into(),
            execution_sha256: "a".repeat(64),
            configuration_sha256: "b".repeat(64),
        }
    }

    fn snapshot(body: &[u8]) -> CandidateAuditSnapshot {
        let skill = b"---\nname: demo\ndescription: Use when reviewing a demo.\n---\n\n# Demo\n";
        let files = vec![
            CandidateAuditSnapshotFile {
                manifest: CandidateFile {
                    path: "SKILL.md".into(),
                    size: skill.len(),
                    sha256: sha256(skill),
                    executable: false,
                },
                bytes: skill.to_vec(),
            },
            CandidateAuditSnapshotFile {
                manifest: CandidateFile {
                    path: "scripts/check.sh".into(),
                    size: body.len(),
                    sha256: sha256(body),
                    executable: true,
                },
                bytes: body.to_vec(),
            },
        ];
        let fingerprint = files
            .iter()
            .map(|file| {
                format!(
                    "{}:{}:{}:{}",
                    file.manifest.path,
                    file.manifest.size,
                    file.manifest.sha256,
                    file.manifest.executable
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        CandidateAuditSnapshot {
            candidate_hash: sha256(fingerprint.as_bytes()),
            files,
        }
    }

    fn adapter(findings: Vec<ReportedScannerFinding>) -> FakeAdapter {
        FakeAdapter {
            identity: Mutex::new(identity()),
            configuration_summary: Mutex::new(
                "Local static rules only; network engines disabled.".into(),
            ),
            findings: Mutex::new(findings),
            parse_calls: AtomicUsize::new(0),
            handling: ScannerDataHandling::LocalOnly,
            omit_last_file: false,
            flip_executable: false,
        }
    }

    fn manager() -> (ExternalScannerManager, Arc<FakeRuntime>) {
        let runtime = Arc::new(FakeRuntime {
            execute_calls: AtomicUsize::new(0),
            output: b"fixture scanner output".to_vec(),
        });
        (ExternalScannerManager::new(runtime.clone()), runtime)
    }

    fn run_scan(
        manager: &ExternalScannerManager,
        adapter: &dyn ExternalScannerAdapter,
        snapshot: &CandidateAuditSnapshot,
        expected_plan_hash: &str,
    ) -> Result<ScannerAuditContribution, ExternalScannerError> {
        manager.run(
            adapter,
            snapshot,
            expected_plan_hash,
            &StaticCancellation(false),
        )
    }

    #[test]
    fn preview_is_side_effect_free_and_binds_identity_candidate_and_files() {
        let (manager, runtime) = manager();
        let adapter = adapter(Vec::new());
        let first = snapshot(b"echo inspect\n");
        let plan = manager.preview(&adapter, &first).unwrap();
        assert_eq!(runtime.execute_calls.load(Ordering::Relaxed), 0);
        assert_eq!(adapter.parse_calls.load(Ordering::Relaxed), 0);
        assert_eq!(plan.candidate_hash, first.candidate_hash);
        assert_eq!(plan.files.len(), 2);
        assert!(!plan.files[0].executable);
        assert!(plan.files[1].executable);
        assert_eq!(
            plan.configuration_summary,
            "Local static rules only; network engines disabled."
        );

        let changed = snapshot(b"echo changed\n");
        let changed_plan = manager.preview(&adapter, &changed).unwrap();
        assert_ne!(plan.plan_hash, changed_plan.plan_hash);
    }

    #[test]
    fn run_rejects_candidate_or_scanner_changes_before_scan() {
        let (manager, runtime) = manager();
        let adapter = adapter(Vec::new());
        let original = snapshot(b"echo inspect\n");
        let plan = manager.preview(&adapter, &original).unwrap();
        let changed = snapshot(b"echo changed\n");
        assert!(matches!(
            run_scan(&manager, &adapter, &changed, &plan.plan_hash),
            Err(ExternalScannerError::StalePlan)
        ));
        assert_eq!(runtime.execute_calls.load(Ordering::Relaxed), 0);
        assert_eq!(adapter.parse_calls.load(Ordering::Relaxed), 0);

        adapter.identity.lock().unwrap().product_version = "2.1.0".into();
        assert!(matches!(
            run_scan(&manager, &adapter, &original, &plan.plan_hash),
            Err(ExternalScannerError::StalePlan)
        ));
        assert_eq!(runtime.execute_calls.load(Ordering::Relaxed), 0);
        assert_eq!(adapter.parse_calls.load(Ordering::Relaxed), 0);

        adapter.identity.lock().unwrap().product_version = "2.0.0".into();
        *adapter.configuration_summary.lock().unwrap() =
            "Local rules changed; network engines disabled.".into();
        assert!(matches!(
            run_scan(&manager, &adapter, &original, &plan.plan_hash),
            Err(ExternalScannerError::StalePlan)
        ));
        assert_eq!(runtime.execute_calls.load(Ordering::Relaxed), 0);
        assert_eq!(adapter.parse_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn cancellation_before_execution_invokes_neither_runtime_nor_parser() {
        let (manager, runtime) = manager();
        let adapter = adapter(Vec::new());
        let snapshot = snapshot(b"echo inspect\n");
        let plan = manager.preview(&adapter, &snapshot).unwrap();

        assert!(matches!(
            manager.run(
                &adapter,
                &snapshot,
                &plan.plan_hash,
                &StaticCancellation(true)
            ),
            Err(ExternalScannerError::Cancelled)
        ));
        assert_eq!(runtime.execute_calls.load(Ordering::Relaxed), 0);
        assert_eq!(adapter.parse_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn run_rebuilds_line_and_file_evidence_and_derives_verdict() {
        let findings = vec![
            ReportedScannerFinding {
                id: "shell-line".into(),
                severity: ScannerSeverity::Warning,
                confidence: ScannerConfidence::Medium,
                title: "Shell instruction".into(),
                explanation: "The script contains a shell instruction.".into(),
                location: ScannerFindingLocation::Lines {
                    path: "scripts/check.sh".into(),
                    start: 2,
                    end: 2,
                },
            },
            ReportedScannerFinding {
                id: "file-rule".into(),
                severity: ScannerSeverity::Blocker,
                confidence: ScannerConfidence::High,
                title: "File-level rule".into(),
                explanation: "The scanner matched the complete file.".into(),
                location: ScannerFindingLocation::File {
                    path: "scripts/check.sh".into(),
                },
            },
        ];
        let (manager, runtime) = manager();
        let adapter = adapter(findings);
        let snapshot = snapshot(b"echo first\necho second\n");
        let plan = manager.preview(&adapter, &snapshot).unwrap();
        let result = run_scan(&manager, &adapter, &snapshot, &plan.plan_hash).unwrap();
        assert_eq!(runtime.execute_calls.load(Ordering::Relaxed), 1);
        assert_eq!(adapter.parse_calls.load(Ordering::Relaxed), 1);
        assert_eq!(result.verdict, "block");
        assert_eq!(result.raw_result_sha256, sha256(b"fixture scanner output"));
        assert!(result.scanned_files[1].executable);
        assert_eq!(result.findings[0].evidence, "echo second");
        assert!(result.findings[1].evidence.contains("SHA-256"));
        assert!(result
            .findings
            .iter()
            .all(|finding| finding.source == "scanner:fixture-static"));
    }

    #[test]
    fn ungrounded_locations_and_scanned_file_mismatches_are_rejected() {
        let (manager, _) = manager();
        let ungrounded_adapter = adapter(vec![ReportedScannerFinding {
            id: "escape".into(),
            severity: ScannerSeverity::Warning,
            confidence: ScannerConfidence::Low,
            title: "Escaped path".into(),
            explanation: "References a path outside the snapshot.".into(),
            location: ScannerFindingLocation::File {
                path: "../outside".into(),
            },
        }]);
        let snapshot = snapshot(b"echo inspect\n");
        let plan = manager.preview(&ungrounded_adapter, &snapshot).unwrap();
        assert!(matches!(
            run_scan(&manager, &ungrounded_adapter, &snapshot, &plan.plan_hash),
            Err(ExternalScannerError::UngroundedEvidence)
        ));

        let mut incomplete = adapter(Vec::new());
        incomplete.omit_last_file = true;
        let plan = manager.preview(&incomplete, &snapshot).unwrap();
        assert!(matches!(
            run_scan(&manager, &incomplete, &snapshot, &plan.plan_hash),
            Err(ExternalScannerError::InvalidReport)
        ));

        let mut wrong_mode = adapter(Vec::new());
        wrong_mode.flip_executable = true;
        let plan = manager.preview(&wrong_mode, &snapshot).unwrap();
        assert!(matches!(
            run_scan(&manager, &wrong_mode, &snapshot, &plan.plan_hash),
            Err(ExternalScannerError::InvalidReport)
        ));
    }

    #[test]
    fn invalid_line_ranges_and_oversized_rebuilt_evidence_are_rejected() {
        let (manager, _) = manager();
        let invalid_line = adapter(vec![ReportedScannerFinding {
            id: "invalid-line".into(),
            severity: ScannerSeverity::Warning,
            confidence: ScannerConfidence::Medium,
            title: "Invalid line".into(),
            explanation: "The scanner returned a line outside the file.".into(),
            location: ScannerFindingLocation::Lines {
                path: "scripts/check.sh".into(),
                start: 0,
                end: 1,
            },
        }]);
        let ordinary_snapshot = snapshot(b"echo inspect\n");
        let plan = manager.preview(&invalid_line, &ordinary_snapshot).unwrap();
        assert!(matches!(
            run_scan(&manager, &invalid_line, &ordinary_snapshot, &plan.plan_hash),
            Err(ExternalScannerError::UngroundedEvidence)
        ));

        let oversized = adapter(vec![ReportedScannerFinding {
            id: "oversized-evidence".into(),
            severity: ScannerSeverity::Info,
            confidence: ScannerConfidence::Low,
            title: "Oversized line".into(),
            explanation: "The scanner referenced one unusually long line.".into(),
            location: ScannerFindingLocation::Lines {
                path: "scripts/check.sh".into(),
                start: 1,
                end: 1,
            },
        }]);
        let snapshot = snapshot(&vec![b'x'; MAX_TEXT + 1]);
        let plan = manager.preview(&oversized, &snapshot).unwrap();
        assert!(matches!(
            run_scan(&manager, &oversized, &snapshot, &plan.plan_hash),
            Err(ExternalScannerError::OutputLimit)
        ));
    }

    #[test]
    fn invalid_snapshot_bytes_and_duplicate_finding_ids_are_rejected() {
        let (manager, _) = manager();
        let finding = ReportedScannerFinding {
            id: "duplicate".into(),
            severity: ScannerSeverity::Info,
            confidence: ScannerConfidence::Low,
            title: "Informational match".into(),
            explanation: "The scanner reported an informational match.".into(),
            location: ScannerFindingLocation::File {
                path: "SKILL.md".into(),
            },
        };
        let adapter = adapter(vec![finding.clone(), finding]);
        let snapshot = snapshot(b"echo inspect\n");
        let plan = manager.preview(&adapter, &snapshot).unwrap();
        assert!(matches!(
            run_scan(&manager, &adapter, &snapshot, &plan.plan_hash),
            Err(ExternalScannerError::InvalidReport)
        ));

        let mut changed = snapshot.clone();
        changed.files[0].bytes.push(b'!');
        assert!(matches!(
            manager.preview(&adapter, &changed),
            Err(ExternalScannerError::InvalidPlan)
        ));
    }

    #[test]
    fn external_data_handling_is_visible_in_the_plan() {
        let (manager, runtime) = manager();
        let mut adapter = adapter(Vec::new());
        adapter.handling = ScannerDataHandling::External {
            endpoint: "https://scanner.example.test/v1".into(),
        };
        let snapshot = snapshot(b"echo inspect\n");
        let plan = manager.preview(&adapter, &snapshot).unwrap();
        assert!(matches!(
            plan.data_handling,
            ScannerDataHandling::External { .. }
        ));
        assert_eq!(runtime.execute_calls.load(Ordering::Relaxed), 0);
        assert_eq!(adapter.parse_calls.load(Ordering::Relaxed), 0);
    }

    #[cfg(unix)]
    #[test]
    fn real_candidate_snapshot_preserves_modes_and_never_executes_candidate_files() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TempDir::new().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(source.join("scripts")).unwrap();
        fs::write(
            source.join("SKILL.md"),
            "---\nname: scanner-fixture\ndescription: Use when testing scanner evidence.\n---\n\n# Scanner fixture\n",
        )
        .unwrap();
        let marker = directory.path().join("candidate-executed");
        fs::write(
            source.join("scripts/check.sh"),
            format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
        )
        .unwrap();
        fs::set_permissions(
            source.join("scripts/check.sh"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();

        let stager = CandidateStager::new(directory.path().join("staging")).unwrap();
        let manifest = stager.stage_local(&source).unwrap();
        let snapshot = stager
            .audit_snapshot(&manifest.session_id, &manifest.candidate_hash)
            .unwrap();
        let (manager, runtime) = manager();
        let adapter = adapter(Vec::new());
        let plan = manager.preview(&adapter, &snapshot).unwrap();
        let result = run_scan(&manager, &adapter, &snapshot, &plan.plan_hash).unwrap();

        assert!(plan
            .files
            .iter()
            .any(|file| file.path == "scripts/check.sh" && file.executable));
        assert!(result
            .scanned_files
            .iter()
            .any(|file| file.path == "scripts/check.sh" && file.executable));
        assert_eq!(runtime.execute_calls.load(Ordering::Relaxed), 1);
        assert!(!marker.exists());
    }
}
