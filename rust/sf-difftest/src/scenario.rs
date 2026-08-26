use crate::{first_divergence, validate_trace, Divergence, SemanticFrame, TraceError};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

pub const SCENARIO_SCHEMA_VERSION: u32 = 2;
pub const EVIDENCE_SCHEMA_VERSION: u32 = 2;
const SHA256_TEXT_LENGTH: usize = 64;

#[derive(Clone, Copy, Debug, Deserialize, Ord, PartialOrd, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureChannel {
    SemanticState,
    ObjectLifecycle,
    DrawCommands,
    SourceResolutionVideo,
    AudioEvents,
    Coverage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceProducer {
    Retail,
    Native,
}

/// The independently observed boundary represented by one evidence frame.
///
/// Logical updates may consume a variable number of source display refreshes.
/// Presentation frames instead use a contiguous elapsed-refresh clock, so
/// pixels and audio cannot be compared at adapter-selected logic boundaries.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum ScenarioClock {
    LogicalUpdate,
    PresentationFrame {
        refresh_rate_numerator: u32,
        refresh_rate_denominator: u32,
    },
}

impl ScenarioClock {
    pub const fn logical_update() -> Self {
        Self::LogicalUpdate
    }

    pub const fn presentation_frame(
        refresh_rate_numerator: u32,
        refresh_rate_denominator: u32,
    ) -> Self {
        Self::PresentationFrame {
            refresh_rate_numerator,
            refresh_rate_denominator,
        }
    }

    fn validate(self, scenario_id: &str) -> Result<(), TraceError> {
        if let Self::PresentationFrame {
            refresh_rate_numerator,
            refresh_rate_denominator,
        } = self
        {
            if refresh_rate_numerator == 0 || refresh_rate_denominator == 0 {
                return Err(TraceError::new(format!(
                    "scenario {scenario_id:?}: presentation refresh rate must be nonzero"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScenarioInputRun {
    pub frames: u64,
    pub input: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScenarioManifest {
    pub schema_version: u32,
    pub id: String,
    pub description: String,
    pub retail_rom_sha256: String,
    pub clock: ScenarioClock,
    pub input_runs: Vec<ScenarioInputRun>,
    pub required_channels: BTreeSet<CaptureChannel>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required_retail_coverage: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required_native_coverage: BTreeSet<String>,
}

impl ScenarioManifest {
    pub fn expected_frames(&self) -> Result<u64, TraceError> {
        self.input_runs.iter().try_fold(0u64, |total, run| {
            total.checked_add(run.frames).ok_or_else(|| {
                TraceError::new(format!("scenario {:?}: input duration overflow", self.id))
            })
        })
    }

    pub fn expected_input(&self, sequence: u64) -> Option<u32> {
        let mut first_sequence = 0u64;
        for run in &self.input_runs {
            let end_sequence = first_sequence.checked_add(run.frames)?;
            if sequence < end_sequence {
                return Some(run.input);
            }
            first_sequence = end_sequence;
        }
        None
    }

    fn validate(&self) -> Result<(), TraceError> {
        if self.schema_version != SCENARIO_SCHEMA_VERSION {
            return Err(TraceError::new(format!(
                "scenario {:?}: schema {}, expected {}",
                self.id, self.schema_version, SCENARIO_SCHEMA_VERSION
            )));
        }
        if self.id.trim().is_empty() {
            return Err(TraceError::new("scenario id must not be empty"));
        }
        if self.description.trim().is_empty() {
            return Err(TraceError::new(format!(
                "scenario {:?}: description must not be empty",
                self.id
            )));
        }
        if self.retail_rom_sha256.len() != SHA256_TEXT_LENGTH
            || !self
                .retail_rom_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(TraceError::new(format!(
                "scenario {:?}: retail ROM SHA-256 must contain exactly {SHA256_TEXT_LENGTH} hexadecimal characters",
                self.id
            )));
        }
        if self.input_runs.is_empty() || self.input_runs.iter().any(|run| run.frames == 0) {
            return Err(TraceError::new(format!(
                "scenario {:?}: input runs must be non-empty",
                self.id
            )));
        }
        if self.required_channels.is_empty() {
            return Err(TraceError::new(format!(
                "scenario {:?}: at least one capture channel is required",
                self.id
            )));
        }
        self.clock.validate(&self.id)?;
        if matches!(self.clock, ScenarioClock::PresentationFrame { .. })
            && !self
                .required_channels
                .contains(&CaptureChannel::SourceResolutionVideo)
            && !self
                .required_channels
                .contains(&CaptureChannel::AudioEvents)
        {
            return Err(TraceError::new(format!(
                "scenario {:?}: presentation-frame evidence must require source video or audio",
                self.id
            )));
        }
        let _ = self.expected_frames()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct NonStrictEvidence {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub substitutions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub normalizations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quarantines: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resynchronizations: Vec<String>,
}

impl NonStrictEvidence {
    fn violations(&self, producer: EvidenceProducer) -> Vec<String> {
        let mut violations = Vec::new();
        for (kind, entries) in [
            ("substitution", &self.substitutions),
            ("normalization", &self.normalizations),
            ("quarantine", &self.quarantines),
            ("resynchronization", &self.resynchronizations),
        ] {
            for entry in entries {
                violations.push(format!("{producer:?} declares {kind}: {entry}"));
            }
        }
        violations
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScenarioEvidence {
    pub schema_version: u32,
    pub scenario_id: String,
    pub producer: EvidenceProducer,
    pub retail_rom_sha256: String,
    pub clock: ScenarioClock,
    pub channels: BTreeSet<CaptureChannel>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub coverage: BTreeSet<String>,
    #[serde(default)]
    pub non_strict: NonStrictEvidence,
    pub frames: Vec<SemanticFrame>,
}

impl ScenarioEvidence {
    fn validate_structure(
        &self,
        manifest: &ScenarioManifest,
        expected_producer: EvidenceProducer,
    ) -> Result<(), TraceError> {
        if self.schema_version != EVIDENCE_SCHEMA_VERSION {
            return Err(TraceError::new(format!(
                "scenario {:?} {expected_producer:?} evidence: schema {}, expected {}",
                manifest.id, self.schema_version, EVIDENCE_SCHEMA_VERSION
            )));
        }
        if self.scenario_id != manifest.id {
            return Err(TraceError::new(format!(
                "scenario id mismatch: manifest {:?}, evidence {:?}",
                manifest.id, self.scenario_id
            )));
        }
        if self.producer != expected_producer {
            return Err(TraceError::new(format!(
                "scenario {:?}: expected {expected_producer:?} evidence, got {:?}",
                manifest.id, self.producer
            )));
        }
        if !self
            .retail_rom_sha256
            .eq_ignore_ascii_case(&manifest.retail_rom_sha256)
        {
            return Err(TraceError::new(format!(
                "scenario {:?}: retail ROM SHA-256 mismatch",
                manifest.id
            )));
        }
        if self.clock != manifest.clock {
            return Err(TraceError::new(format!(
                "scenario {:?} {expected_producer:?}: evidence clock {:?}, expected {:?}",
                manifest.id, self.clock, manifest.clock
            )));
        }
        validate_trace(
            &self.frames,
            &format!("scenario {:?} {expected_producer:?} evidence", manifest.id),
        )?;
        let mut previous_source_frame = None;
        for (index, frame) in self.frames.iter().enumerate() {
            let sequence = index as u64;
            if frame.sequence != sequence {
                return Err(TraceError::new(format!(
                    "scenario {:?} {expected_producer:?}: frame index {index} has sequence {}, expected {sequence}",
                    manifest.id, frame.sequence
                )));
            }
            let expected_input = manifest.expected_input(sequence).ok_or_else(|| {
                TraceError::new(format!(
                    "scenario {:?} {expected_producer:?}: sequence {sequence} exceeds manifest duration",
                    manifest.id
                ))
            })?;
            if frame.input != expected_input {
                return Err(TraceError::new(format!(
                    "scenario {:?} {expected_producer:?}: sequence {sequence} input {}, expected {expected_input}",
                    manifest.id, frame.input
                )));
            }
            if previous_source_frame.is_some_and(|previous| previous >= frame.source_frame) {
                return Err(TraceError::new(format!(
                    "scenario {:?} {expected_producer:?}: source frame {} at sequence {sequence} is not strictly increasing",
                    manifest.id, frame.source_frame
                )));
            }
            if matches!(manifest.clock, ScenarioClock::PresentationFrame { .. })
                && frame.source_frame != sequence
            {
                return Err(TraceError::new(format!(
                    "scenario {:?} {expected_producer:?}: presentation sequence {sequence} has elapsed source frame {}, expected {sequence}",
                    manifest.id, frame.source_frame
                )));
            }
            previous_source_frame = Some(frame.source_frame);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConformanceReport {
    pub scenario_id: String,
    pub strict_pass: bool,
    pub certified_frames: u64,
    pub expected_frames: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_divergence: Option<Divergence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub violations: Vec<String>,
    pub retail_coverage_points: usize,
    pub native_coverage_points: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_retail_coverage: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_native_coverage: Vec<String>,
}

pub fn compare_scenario(
    manifest: &ScenarioManifest,
    retail: &ScenarioEvidence,
    native: &ScenarioEvidence,
) -> Result<ConformanceReport, TraceError> {
    manifest.validate()?;
    retail.validate_structure(manifest, EvidenceProducer::Retail)?;
    native.validate_structure(manifest, EvidenceProducer::Native)?;

    let expected_frames = manifest.expected_frames()?;
    let first_divergence = first_divergence(&retail.frames, &native.frames)?;
    let certified_frames = first_divergence
        .as_ref()
        .map_or(
            retail.frames.len().min(native.frames.len()) as u64,
            |divergence| divergence.sequence,
        )
        .min(expected_frames);

    let mut violations = retail.non_strict.violations(retail.producer);
    violations.extend(native.non_strict.violations(native.producer));

    for channel in &manifest.required_channels {
        if !retail.channels.contains(channel) {
            violations.push(format!("Retail evidence is missing channel {channel:?}"));
        }
        if !native.channels.contains(channel) {
            violations.push(format!("Native evidence is missing channel {channel:?}"));
        }
    }

    if manifest
        .required_channels
        .contains(&CaptureChannel::SourceResolutionVideo)
    {
        require_frame_hashes(&mut violations, retail, "video", |frame| {
            frame.video.is_some()
        });
        require_frame_hashes(&mut violations, native, "video", |frame| {
            frame.video.is_some()
        });
    }
    if manifest
        .required_channels
        .contains(&CaptureChannel::AudioEvents)
    {
        require_frame_hashes(&mut violations, retail, "audio", |frame| {
            frame.audio.is_some()
        });
        require_frame_hashes(&mut violations, native, "audio", |frame| {
            frame.audio.is_some()
        });
    }

    if retail.frames.len() as u64 != expected_frames {
        violations.push(format!(
            "Retail evidence has {} frames; manifest requires {expected_frames}",
            retail.frames.len()
        ));
    }
    if native.frames.len() as u64 != expected_frames {
        violations.push(format!(
            "Native evidence has {} frames; manifest requires {expected_frames}",
            native.frames.len()
        ));
    }

    let missing_retail_coverage =
        missing_coverage(&manifest.required_retail_coverage, &retail.coverage);
    let missing_native_coverage =
        missing_coverage(&manifest.required_native_coverage, &native.coverage);
    if !missing_retail_coverage.is_empty() {
        violations.push(format!(
            "Retail evidence misses {} required coverage points",
            missing_retail_coverage.len()
        ));
    }
    if !missing_native_coverage.is_empty() {
        violations.push(format!(
            "Native evidence misses {} required coverage points",
            missing_native_coverage.len()
        ));
    }

    let strict_pass =
        first_divergence.is_none() && violations.is_empty() && certified_frames == expected_frames;
    Ok(ConformanceReport {
        scenario_id: manifest.id.clone(),
        strict_pass,
        certified_frames,
        expected_frames,
        first_divergence,
        violations,
        retail_coverage_points: retail.coverage.len(),
        native_coverage_points: native.coverage.len(),
        missing_retail_coverage,
        missing_native_coverage,
    })
}

fn require_frame_hashes(
    violations: &mut Vec<String>,
    evidence: &ScenarioEvidence,
    name: &str,
    present: impl Fn(&SemanticFrame) -> bool,
) {
    if let Some(frame) = evidence.frames.iter().find(|frame| !present(frame)) {
        violations.push(format!(
            "{:?} evidence is missing {name} observation at sequence {}",
            evidence.producer, frame.sequence
        ));
    }
}

fn missing_coverage(required: &BTreeSet<String>, actual: &BTreeSet<String>) -> Vec<String> {
    required.difference(actual).cloned().collect()
}

pub fn read_scenario_manifest(path: impl AsRef<Path>) -> Result<ScenarioManifest, TraceError> {
    read_json(path, "scenario manifest")
}

pub fn read_scenario_evidence(path: impl AsRef<Path>) -> Result<ScenarioEvidence, TraceError> {
    read_json(path, "scenario evidence")
}

pub fn write_scenario_manifest(
    path: impl AsRef<Path>,
    manifest: &ScenarioManifest,
) -> Result<(), TraceError> {
    write_json(path, manifest, "scenario manifest")
}

pub fn write_scenario_evidence(
    path: impl AsRef<Path>,
    evidence: &ScenarioEvidence,
) -> Result<(), TraceError> {
    write_json(path, evidence, "scenario evidence")
}

fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
    description: &str,
) -> Result<T, TraceError> {
    let path = path.as_ref();
    let file = File::open(path)
        .map_err(|error| TraceError::new(format!("{}: {error}", path.display())))?;
    serde_json::from_reader(BufReader::new(file))
        .map_err(|error| TraceError::new(format!("{} {}: {error}", path.display(), description)))
}

fn write_json<T: Serialize>(
    path: impl AsRef<Path>,
    value: &T,
    description: &str,
) -> Result<(), TraceError> {
    let path = path.as_ref();
    let file = File::create(path)
        .map_err(|error| TraceError::new(format!("{}: {error}", path.display())))?;
    serde_json::to_writer_pretty(BufWriter::new(file), value)
        .map_err(|error| TraceError::new(format!("{} {}: {error}", path.display(), description)))
}
