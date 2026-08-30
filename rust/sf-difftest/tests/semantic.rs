use sf_difftest::{
    first_divergence, read_jsonl_from, semantic_frame_sha256, HashObservation, SemanticEvent,
    SemanticFrame, SemanticObject,
};
use std::io::Cursor;

fn frame(sequence: u64) -> SemanticFrame {
    SemanticFrame::new(sequence, sequence * 2, 0)
        .with_field("camera.yaw", 90i16)
        .with_object(
            SemanticObject::new("enemy-3", "fighter")
                .with_field("position.x", 120i16)
                .with_field("position.z", -800i16),
        )
        .with_event(SemanticEvent::new("sound").with_field("cue", "laser"))
}

#[test]
fn identical_traces_have_no_divergence() {
    let mut expected = frame(1);
    expected.video = Some(HashObservation {
        item_count: 224 * 256,
        hash: 17,
    });
    assert_eq!(first_divergence(&[expected.clone()], &[expected]), Ok(None));
}

#[test]
fn reports_earliest_named_field() {
    let expected = [frame(1), frame(2)];
    let candidate = [frame(1), frame(2).with_field("camera.yaw", 91i16)];
    let divergence = first_divergence(&expected, &candidate)
        .expect("valid traces")
        .expect("divergence");
    assert_eq!(divergence.sequence, 2);
    assert_eq!(divergence.path, "fields.camera.yaw");
    assert_eq!(divergence.reference, "Integer(90)");
    assert_eq!(divergence.candidate, "Integer(91)");
}

#[test]
fn objects_are_aligned_by_stable_identity() {
    let expected =
        frame(1).with_object(SemanticObject::new("enemy-1", "fighter").with_field("health", 12i16));
    let mut candidate = expected.clone();
    candidate.objects.reverse();
    assert_eq!(first_divergence(&[expected], &[candidate]), Ok(None));
}

#[test]
fn reports_missing_frame_without_cascading() {
    let expected = [frame(1), frame(2), frame(3)];
    let candidate = [frame(1), frame(3)];
    let divergence = first_divergence(&expected, &candidate)
        .expect("valid traces")
        .expect("divergence");
    assert_eq!(divergence.sequence, 2);
    assert_eq!(divergence.path, "frame");
    assert_eq!(divergence.reference, "present");
    assert_eq!(divergence.candidate, "missing");
}

#[test]
fn parser_reports_line_and_rejects_duplicate_object_identity() {
    let trace = concat!(
        "{\"schema_version\":1,\"sequence\":1,\"source_frame\":1,\"input\":0,",
        "\"objects\":[{\"identity\":\"wingman\",\"kind\":\"fighter\"},",
        "{\"identity\":\"wingman\",\"kind\":\"fighter\"}]}\n"
    );
    let error = read_jsonl_from(Cursor::new(trace), "duplicate.jsonl")
        .expect_err("duplicate identity must fail");
    assert!(error
        .to_string()
        .contains("sequence 1 repeats object identity \"wingman\""));

    let error = read_jsonl_from(Cursor::new("{}\nnot-json\n"), "broken.jsonl")
        .expect_err("invalid JSON must fail");
    assert!(error.to_string().contains("broken.jsonl:1:"));
}

#[test]
fn semantic_frame_fingerprint_covers_named_state_and_is_stable() {
    let expected = frame(1);
    let changed = frame(1).with_field("camera.yaw", 91i16);
    let mut reordered = expected.clone();
    reordered.objects.reverse();

    assert_eq!(
        semantic_frame_sha256(&expected).expect("fingerprint"),
        semantic_frame_sha256(&expected).expect("repeat fingerprint")
    );
    assert_eq!(
        semantic_frame_sha256(&expected).expect("fingerprint"),
        semantic_frame_sha256(&reordered).expect("reordered fingerprint")
    );
    assert_ne!(
        semantic_frame_sha256(&expected).expect("fingerprint"),
        semantic_frame_sha256(&changed).expect("changed fingerprint")
    );
}
