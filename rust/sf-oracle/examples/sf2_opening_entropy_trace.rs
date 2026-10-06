//! Compare independent Mesen opening snapshots with native actor execution.
//! Only the initial random seed and refresh ordering are supplied to native
//! state. Expected actor poses, camera and final random states are assertions.
use sf2_game::intro_controller::{IntroColor, OpeningScenePalette, INTRO_PALETTE_COLORS};
use sf2_game::intro_scene::OpeningScene;
use sf2_game::RandomState;
use std::collections::BTreeMap;

fn numbers<T: std::str::FromStr>(text: &str) -> Vec<T>
where
    T::Err: std::fmt::Debug,
{
    text.split(',')
        .map(|value| value.parse().unwrap())
        .collect()
}

fn named_fields<'a>(fields: impl Iterator<Item = &'a str>) -> BTreeMap<&'a str, &'a str> {
    let mut parsed = BTreeMap::new();
    for field in fields {
        let (key, value) = field.split_once('=').expect("named trace field");
        assert!(parsed.insert(key, value).is_none(), "duplicate trace field");
    }
    parsed
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("independent Mesen entropy trace path");
    let trace = std::fs::read_to_string(path).expect("read Mesen trace");
    let mut scene = None;
    let mut refreshes = Vec::new();
    let mut updates = 0;
    let mut refresh_count = 0;
    let mut intra_actor_refreshes = 0;
    let mut summary_seen = false;
    for line in trace.lines() {
        assert!(!summary_seen, "records after final summary");
        let mut fields = line.split_whitespace();
        let kind = fields.next().expect("nonempty trace record");
        let fields = named_fields(fields);
        match kind {
            "start" => {
                assert!(scene.is_none(), "repeated initial state");
                scene = Some(OpeningScene::new(
                    RandomState::new(numbers(fields["random"]).try_into().unwrap()),
                    // Palette is outside this actor/camera comparison. No
                    // subsequent source state is assigned to the native scene.
                    OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]),
                ));
            }
            "refresh" => {
                assert!(scene.is_some());
                assert_eq!(fields["update"].parse::<usize>().unwrap(), updates + 1);
                refreshes.push(fields["draws"].parse::<usize>().unwrap());
                refresh_count += 1;
                intra_actor_refreshes +=
                    usize::from(fields["actor_draws"].parse::<usize>().unwrap() > 0);
            }
            "complete" => {
                updates += 1;
                assert_eq!(fields["update"].parse::<usize>().unwrap(), updates);
                let scene = scene.as_mut().expect("initial state precedes updates");
                let events = scene
                    .tick_with_random_refreshes(&refreshes)
                    .expect("opening capacity");
                assert_eq!(
                    events.random_draws,
                    fields["draws"].parse::<usize>().unwrap(),
                    "random draw count at update {updates}"
                );
                refreshes.clear();
                assert_eq!(
                    scene.random().bytes().to_vec(),
                    numbers::<u8>(fields["random"]),
                    "random state at update {updates}"
                );
                let slots: Vec<_> = scene.actors().map(|(id, _)| id.index()).collect();
                assert_eq!(
                    slots,
                    numbers::<usize>(fields["slots"]),
                    "slot order at update {updates}"
                );
                let poses: Vec<_> = scene
                    .actors()
                    .filter(|(id, _)| id.index() > 1)
                    .map(|(id, actor)| {
                        let pose = actor.pose();
                        vec![
                            id.index() as i64,
                            i64::from(pose.position.x),
                            i64::from(pose.position.y),
                            i64::from(pose.position.z),
                            i64::from(pose.rotation.pitch.units()),
                            i64::from(pose.rotation.yaw.units()),
                            i64::from(pose.rotation.roll.units()),
                        ]
                    })
                    .collect();
                let expected_poses: Vec<_> =
                    fields["poses"].split(';').map(numbers::<i64>).collect();
                assert_eq!(poses, expected_poses, "actor poses at update {updates}");
                let view = scene.camera();
                let camera = vec![
                    i64::from(view.position.x),
                    i64::from(view.position.y),
                    i64::from(view.position.z),
                    i64::from(view.angles.pitch),
                    i64::from(view.angles.yaw),
                    i64::from(view.angles.roll),
                ];
                assert_eq!(
                    camera,
                    numbers::<i64>(fields["camera"]),
                    "camera at update {updates}"
                );
            }
            "summary" => {
                assert_eq!(updates, fields["updates"].parse::<usize>().unwrap());
                assert_eq!(refresh_count, fields["refreshes"].parse::<usize>().unwrap());
                assert_eq!(
                    intra_actor_refreshes,
                    fields["intra_actor_refreshes"].parse::<usize>().unwrap()
                );
                assert!(intra_actor_refreshes > 0);
                assert!(refreshes.is_empty());
                assert!(updates > 0);
                summary_seen = true;
            }
            _ => panic!("unrecognized trace record {kind}"),
        }
    }
    assert!(summary_seen, "truncated trace without summary");
    println!("Independent Mesen opening comparison passed: {updates} updates, {refresh_count} ordered entropy refreshes; actor slots, poses, camera and RNG. Palette, pixels, audio and autonomous timing are outside this comparison.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_distinct_named_fields() {
        assert_eq!(
            named_fields("draws=4 random=1,2,3,4".split_whitespace())["draws"],
            "4"
        );
    }

    #[test]
    #[should_panic(expected = "duplicate trace field")]
    fn reject_duplicate_named_fields() {
        named_fields("draws=4 draws=5".split_whitespace());
    }

    #[test]
    #[should_panic(expected = "named trace field")]
    fn reject_missing_field_name() {
        named_fields("draws=4 5".split_whitespace());
    }
}
