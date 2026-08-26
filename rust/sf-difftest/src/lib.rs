//! Deterministic semantic trace format and first-divergence comparison.
//!
//! The trace vocabulary describes game concepts rather than either
//! implementation's storage. Retail-oracle adapters and native-port adapters
//! independently produce these records, which keeps source-machine details out
//! of the port while still allowing frame-by-frame comparison.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

mod scenario;
mod source_video;

pub use scenario::{
    compare_scenario, read_scenario_evidence, read_scenario_manifest, write_scenario_evidence,
    write_scenario_manifest, CaptureChannel, ConformanceReport, EvidenceProducer,
    NonStrictEvidence, ScenarioClock, ScenarioEvidence, ScenarioInputRun, ScenarioManifest,
    EVIDENCE_SCHEMA_VERSION, SCENARIO_SCHEMA_VERSION,
};
pub use source_video::{
    compare_source_rgb, hash_rgb, read_source_rgb_ppm, write_source_rgb_ppm, SourceVideoDivergence,
    SOURCE_FRAME_HEIGHT, SOURCE_FRAME_RGB_BYTES, SOURCE_FRAME_WIDTH,
};

pub const TRACE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum SemanticValue {
    Bool(bool),
    Integer(i64),
    Text(String),
}

macro_rules! integer_value_from {
    ($($integer:ty),+ $(,)?) => {
        $(
            impl From<$integer> for SemanticValue {
                fn from(value: $integer) -> Self {
                    Self::Integer(i64::from(value))
                }
            }
        )+
    };
}

integer_value_from!(i8, i16, i32, u8, u16, u32);

impl From<i64> for SemanticValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<bool> for SemanticValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<String> for SemanticValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for SemanticValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SemanticObject {
    pub identity: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, SemanticValue>,
}

impl SemanticObject {
    pub fn new(identity: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            identity: identity.into(),
            kind: kind.into(),
            fields: BTreeMap::new(),
        }
    }

    pub fn with_field(mut self, name: impl Into<String>, value: impl Into<SemanticValue>) -> Self {
        self.fields.insert(name.into(), value.into());
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SemanticEvent {
    pub kind: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, SemanticValue>,
}

impl SemanticEvent {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            fields: BTreeMap::new(),
        }
    }

    pub fn with_field(mut self, name: impl Into<String>, value: impl Into<SemanticValue>) -> Self {
        self.fields.insert(name.into(), value.into());
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct HashObservation {
    pub item_count: u64,
    pub hash: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SemanticFrame {
    pub schema_version: u32,
    pub sequence: u64,
    pub source_frame: u64,
    pub input: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, SemanticValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<SemanticObject>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<SemanticEvent>,
    /// Ordered renderer-boundary commands. Unlike `objects`, command order is
    /// observable and therefore must not be normalized by identity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub draw_commands: Vec<SemanticEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<HashObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<HashObservation>,
}

impl SemanticFrame {
    pub fn new(sequence: u64, source_frame: u64, input: u32) -> Self {
        Self {
            schema_version: TRACE_SCHEMA_VERSION,
            sequence,
            source_frame,
            input,
            fields: BTreeMap::new(),
            objects: Vec::new(),
            events: Vec::new(),
            draw_commands: Vec::new(),
            video: None,
            audio: None,
        }
    }

    pub fn with_field(mut self, name: impl Into<String>, value: impl Into<SemanticValue>) -> Self {
        self.fields.insert(name.into(), value.into());
        self
    }

    pub fn with_object(mut self, object: SemanticObject) -> Self {
        self.objects.push(object);
        self
    }

    pub fn with_event(mut self, event: SemanticEvent) -> Self {
        self.events.push(event);
        self
    }

    pub fn with_draw_command(mut self, command: SemanticEvent) -> Self {
        self.draw_commands.push(command);
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Divergence {
    pub sequence: u64,
    pub path: String,
    pub reference: String,
    pub candidate: String,
}

impl fmt::Display for Divergence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "first divergence at sequence {}: {} (reference {}, candidate {})",
            self.sequence, self.path, self.reference, self.candidate
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceError(String);

impl TraceError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for TraceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TraceError {}

pub fn read_jsonl(path: impl AsRef<Path>) -> Result<Vec<SemanticFrame>, TraceError> {
    let path = path.as_ref();
    let file = File::open(path)
        .map_err(|error| TraceError::new(format!("{}: {error}", path.display())))?;
    read_jsonl_from(BufReader::new(file), &path.display().to_string())
}

pub fn read_jsonl_from(
    reader: impl BufRead,
    source_name: &str,
) -> Result<Vec<SemanticFrame>, TraceError> {
    let mut frames = Vec::new();
    for (line_index, line) in reader.lines().enumerate() {
        let line_number = line_index + 1;
        let line =
            line.map_err(|error| TraceError::new(format!("{source_name}:{line_number}: {error}")))?;
        if line.trim().is_empty() {
            continue;
        }
        let frame = serde_json::from_str(&line)
            .map_err(|error| TraceError::new(format!("{source_name}:{line_number}: {error}")))?;
        frames.push(frame);
    }
    validate_trace(&frames, source_name)?;
    Ok(frames)
}

pub fn write_jsonl(path: impl AsRef<Path>, frames: &[SemanticFrame]) -> Result<(), TraceError> {
    let path = path.as_ref();
    validate_trace(frames, &path.display().to_string())?;
    let file = File::create(path)
        .map_err(|error| TraceError::new(format!("{}: {error}", path.display())))?;
    let mut writer = BufWriter::new(file);
    for frame in frames {
        serde_json::to_writer(&mut writer, frame)
            .map_err(|error| TraceError::new(format!("{}: {error}", path.display())))?;
        writer
            .write_all(b"\n")
            .map_err(|error| TraceError::new(format!("{}: {error}", path.display())))?;
    }
    writer
        .flush()
        .map_err(|error| TraceError::new(format!("{}: {error}", path.display())))
}

pub fn first_divergence(
    reference: &[SemanticFrame],
    candidate: &[SemanticFrame],
) -> Result<Option<Divergence>, TraceError> {
    validate_trace(reference, "reference trace")?;
    validate_trace(candidate, "candidate trace")?;

    let mut reference_index = 0;
    let mut candidate_index = 0;
    while reference_index < reference.len() && candidate_index < candidate.len() {
        let reference_frame = &reference[reference_index];
        let candidate_frame = &candidate[candidate_index];
        if reference_frame.sequence < candidate_frame.sequence {
            return Ok(Some(missing_frame(reference_frame.sequence, true)));
        }
        if candidate_frame.sequence < reference_frame.sequence {
            return Ok(Some(missing_frame(candidate_frame.sequence, false)));
        }
        if let Some(divergence) = compare_frame(reference_frame, candidate_frame) {
            return Ok(Some(divergence));
        }
        reference_index += 1;
        candidate_index += 1;
    }

    if let Some(frame) = reference.get(reference_index) {
        return Ok(Some(missing_frame(frame.sequence, true)));
    }
    if let Some(frame) = candidate.get(candidate_index) {
        return Ok(Some(missing_frame(frame.sequence, false)));
    }
    Ok(None)
}

pub(crate) fn validate_trace(
    frames: &[SemanticFrame],
    source_name: &str,
) -> Result<(), TraceError> {
    let mut previous_sequence = None;
    for frame in frames {
        if frame.schema_version != TRACE_SCHEMA_VERSION {
            return Err(TraceError::new(format!(
                "{source_name}: sequence {} uses schema {}, expected {}",
                frame.sequence, frame.schema_version, TRACE_SCHEMA_VERSION
            )));
        }
        if previous_sequence.is_some_and(|previous| previous >= frame.sequence) {
            return Err(TraceError::new(format!(
                "{source_name}: sequence {} is not strictly increasing",
                frame.sequence
            )));
        }
        previous_sequence = Some(frame.sequence);

        let mut identities = BTreeSet::new();
        for object in &frame.objects {
            if !identities.insert(&object.identity) {
                return Err(TraceError::new(format!(
                    "{source_name}: sequence {} repeats object identity {:?}",
                    frame.sequence, object.identity
                )));
            }
        }
    }
    Ok(())
}

fn missing_frame(sequence: u64, missing_candidate: bool) -> Divergence {
    Divergence {
        sequence,
        path: "frame".to_owned(),
        reference: if missing_candidate {
            "present".to_owned()
        } else {
            "missing".to_owned()
        },
        candidate: if missing_candidate {
            "missing".to_owned()
        } else {
            "present".to_owned()
        },
    }
}

fn compare_frame(reference: &SemanticFrame, candidate: &SemanticFrame) -> Option<Divergence> {
    let sequence = reference.sequence;
    scalar_divergence(
        sequence,
        "frame.source_frame",
        reference.source_frame,
        candidate.source_frame,
    )
    .or_else(|| scalar_divergence(sequence, "frame.input", reference.input, candidate.input))
    .or_else(|| compare_fields(sequence, "fields", &reference.fields, &candidate.fields))
    .or_else(|| compare_objects(sequence, &reference.objects, &candidate.objects))
    .or_else(|| compare_events(sequence, &reference.events, &candidate.events))
    .or_else(|| {
        compare_ordered_commands(
            sequence,
            "draw_commands",
            &reference.draw_commands,
            &candidate.draw_commands,
        )
    })
    .or_else(|| compare_hash(sequence, "video", &reference.video, &candidate.video))
    .or_else(|| compare_hash(sequence, "audio", &reference.audio, &candidate.audio))
}

fn scalar_divergence<T: fmt::Debug + PartialEq>(
    sequence: u64,
    path: &str,
    reference: T,
    candidate: T,
) -> Option<Divergence> {
    (reference != candidate).then(|| Divergence {
        sequence,
        path: path.to_owned(),
        reference: format!("{reference:?}"),
        candidate: format!("{candidate:?}"),
    })
}

fn compare_fields(
    sequence: u64,
    prefix: &str,
    reference: &BTreeMap<String, SemanticValue>,
    candidate: &BTreeMap<String, SemanticValue>,
) -> Option<Divergence> {
    let names: BTreeSet<_> = reference.keys().chain(candidate.keys()).collect();
    for name in names {
        let reference_value = reference.get(name);
        let candidate_value = candidate.get(name);
        if reference_value != candidate_value {
            return Some(Divergence {
                sequence,
                path: format!("{prefix}.{name}"),
                reference: format_optional(reference_value),
                candidate: format_optional(candidate_value),
            });
        }
    }
    None
}

fn compare_objects(
    sequence: u64,
    reference: &[SemanticObject],
    candidate: &[SemanticObject],
) -> Option<Divergence> {
    let reference: BTreeMap<_, _> = reference
        .iter()
        .map(|object| (&object.identity, object))
        .collect();
    let candidate: BTreeMap<_, _> = candidate
        .iter()
        .map(|object| (&object.identity, object))
        .collect();
    let identities: BTreeSet<_> = reference.keys().chain(candidate.keys()).copied().collect();
    for identity in identities {
        let prefix = format!("objects[{identity:?}]");
        let Some(reference_object) = reference.get(identity) else {
            return Some(Divergence {
                sequence,
                path: prefix,
                reference: "missing".to_owned(),
                candidate: "present".to_owned(),
            });
        };
        let Some(candidate_object) = candidate.get(identity) else {
            return Some(Divergence {
                sequence,
                path: prefix,
                reference: "present".to_owned(),
                candidate: "missing".to_owned(),
            });
        };
        if let Some(divergence) = scalar_divergence(
            sequence,
            &format!("{prefix}.kind"),
            &reference_object.kind,
            &candidate_object.kind,
        ) {
            return Some(divergence);
        }
        if let Some(divergence) = compare_fields(
            sequence,
            &format!("{prefix}.fields"),
            &reference_object.fields,
            &candidate_object.fields,
        ) {
            return Some(divergence);
        }
    }
    None
}

fn compare_events(
    sequence: u64,
    reference: &[SemanticEvent],
    candidate: &[SemanticEvent],
) -> Option<Divergence> {
    let common = reference.len().min(candidate.len());
    for index in 0..common {
        let prefix = format!("events[{index}]");
        if let Some(divergence) = scalar_divergence(
            sequence,
            &format!("{prefix}.kind"),
            &reference[index].kind,
            &candidate[index].kind,
        ) {
            return Some(divergence);
        }
        if let Some(divergence) = compare_fields(
            sequence,
            &format!("{prefix}.fields"),
            &reference[index].fields,
            &candidate[index].fields,
        ) {
            return Some(divergence);
        }
    }
    scalar_divergence(sequence, "events.length", reference.len(), candidate.len())
}

fn compare_ordered_commands(
    sequence: u64,
    name: &str,
    reference: &[SemanticEvent],
    candidate: &[SemanticEvent],
) -> Option<Divergence> {
    let common = reference.len().min(candidate.len());
    for index in 0..common {
        let prefix = format!("{name}[{index}]");
        if let Some(divergence) = scalar_divergence(
            sequence,
            &format!("{prefix}.kind"),
            &reference[index].kind,
            &candidate[index].kind,
        ) {
            return Some(divergence);
        }
        if let Some(divergence) = compare_fields(
            sequence,
            &format!("{prefix}.fields"),
            &reference[index].fields,
            &candidate[index].fields,
        ) {
            return Some(divergence);
        }
    }
    scalar_divergence(
        sequence,
        &format!("{name}.length"),
        reference.len(),
        candidate.len(),
    )
}

fn compare_hash(
    sequence: u64,
    name: &str,
    reference: &Option<HashObservation>,
    candidate: &Option<HashObservation>,
) -> Option<Divergence> {
    match (reference, candidate) {
        (Some(reference), Some(candidate)) => scalar_divergence(
            sequence,
            &format!("{name}.item_count"),
            reference.item_count,
            candidate.item_count,
        )
        .or_else(|| {
            scalar_divergence(
                sequence,
                &format!("{name}.hash"),
                reference.hash,
                candidate.hash,
            )
        }),
        (None, None) => None,
        _ => Some(Divergence {
            sequence,
            path: name.to_owned(),
            reference: if reference.is_some() {
                "present"
            } else {
                "missing"
            }
            .to_owned(),
            candidate: if candidate.is_some() {
                "present"
            } else {
                "missing"
            }
            .to_owned(),
        }),
    }
}

fn format_optional(value: Option<&SemanticValue>) -> String {
    value.map_or_else(|| "missing".to_owned(), |value| format!("{value:?}"))
}
