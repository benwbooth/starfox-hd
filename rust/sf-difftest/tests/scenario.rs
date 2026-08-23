use sf_difftest::{
    compare_scenario, CaptureChannel, EvidenceProducer, HashObservation, NonStrictEvidence,
    ScenarioEvidence, ScenarioInputRun, ScenarioManifest, SemanticEvent, SemanticFrame,
    EVIDENCE_SCHEMA_VERSION, SCENARIO_SCHEMA_VERSION,
};
use std::collections::BTreeSet;

const RETAIL_ROM_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const IDLE_INPUT: u32 = 0;
const FIRE_INPUT: u32 = 1;
const IDLE_FRAMES: u64 = 2;
const FIRE_FRAMES: u64 = 1;

fn all_channels() -> BTreeSet<CaptureChannel> {
    [
        CaptureChannel::SemanticState,
        CaptureChannel::ObjectLifecycle,
        CaptureChannel::DrawCommands,
        CaptureChannel::SourceResolutionVideo,
        CaptureChannel::AudioEvents,
        CaptureChannel::Coverage,
    ]
    .into_iter()
    .collect()
}

fn manifest() -> ScenarioManifest {
    ScenarioManifest {
        schema_version: SCENARIO_SCHEMA_VERSION,
        id: "pilot".to_owned(),
        description: "strict conformance pilot".to_owned(),
        retail_rom_sha256: RETAIL_ROM_SHA256.to_owned(),
        input_runs: vec![
            ScenarioInputRun {
                frames: IDLE_FRAMES,
                input: IDLE_INPUT,
            },
            ScenarioInputRun {
                frames: FIRE_FRAMES,
                input: FIRE_INPUT,
            },
        ],
        required_channels: all_channels(),
        required_retail_coverage: ["cpu:title".to_owned()].into_iter().collect(),
        required_native_coverage: ["native:title".to_owned()].into_iter().collect(),
    }
}

fn frames() -> Vec<SemanticFrame> {
    [IDLE_INPUT, IDLE_INPUT, FIRE_INPUT]
        .into_iter()
        .enumerate()
        .map(|(sequence, input)| {
            let mut frame = SemanticFrame::new(sequence as u64, sequence as u64, input)
                .with_field("camera.mode", "title")
                .with_event(SemanticEvent::new("object-active").with_field("identity", "arwing"))
                .with_draw_command(
                    SemanticEvent::new("mesh")
                        .with_field("shape", "arwing")
                        .with_field("rotation.y", sequence as i32),
                );
            frame.video = Some(HashObservation {
                item_count: 256 * 224,
                hash: sequence as u64,
            });
            frame.audio = Some(HashObservation {
                item_count: 0,
                hash: 0,
            });
            frame
        })
        .collect()
}

fn evidence(producer: EvidenceProducer) -> ScenarioEvidence {
    let coverage = match producer {
        EvidenceProducer::Retail => ["cpu:title".to_owned()].into_iter().collect(),
        EvidenceProducer::Native => ["native:title".to_owned()].into_iter().collect(),
    };
    ScenarioEvidence {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        scenario_id: "pilot".to_owned(),
        producer,
        retail_rom_sha256: RETAIL_ROM_SHA256.to_owned(),
        channels: all_channels(),
        coverage,
        non_strict: NonStrictEvidence::default(),
        frames: frames(),
    }
}

#[test]
fn strict_scenario_reports_complete_certification() {
    let report = compare_scenario(
        &manifest(),
        &evidence(EvidenceProducer::Retail),
        &evidence(EvidenceProducer::Native),
    )
    .expect("valid scenario");
    assert!(report.strict_pass);
    assert_eq!(report.certified_frames, IDLE_FRAMES + FIRE_FRAMES);
    assert_eq!(report.first_divergence, None);
    assert!(report.violations.is_empty());
}

#[test]
fn ordered_draw_transform_is_the_earliest_divergence() {
    let retail = evidence(EvidenceProducer::Retail);
    let mut native = evidence(EvidenceProducer::Native);
    native.frames[1].draw_commands[0]
        .fields
        .insert("rotation.y".to_owned(), 99i32.into());

    let report = compare_scenario(&manifest(), &retail, &native).expect("valid scenario");
    assert!(!report.strict_pass);
    assert_eq!(report.certified_frames, 1);
    assert_eq!(
        report.first_divergence.expect("draw divergence").path,
        "draw_commands[0].fields.rotation.y"
    );
}

#[test]
fn any_declared_false_green_mechanism_fails_strictly() {
    let retail = evidence(EvidenceProducer::Retail);
    let mut native = evidence(EvidenceProducer::Native);
    native
        .non_strict
        .quarantines
        .push("copied retail objects for one frame".to_owned());

    let report = compare_scenario(&manifest(), &retail, &native).expect("valid scenario");
    assert!(!report.strict_pass);
    assert_eq!(report.certified_frames, IDLE_FRAMES + FIRE_FRAMES);
    assert!(report.violations[0].contains("quarantine"));
}

#[test]
fn manifest_input_and_coverage_are_enforced() {
    let retail = evidence(EvidenceProducer::Retail);
    let mut native = evidence(EvidenceProducer::Native);
    native.frames[2].input = IDLE_INPUT;
    let error = compare_scenario(&manifest(), &retail, &native).expect_err("wrong input must fail");
    assert!(error.to_string().contains("sequence 2 input"));

    let mut native = evidence(EvidenceProducer::Native);
    native.coverage.clear();
    let report = compare_scenario(&manifest(), &retail, &native).expect("valid structure");
    assert!(!report.strict_pass);
    assert_eq!(report.missing_native_coverage, ["native:title"]);
}
