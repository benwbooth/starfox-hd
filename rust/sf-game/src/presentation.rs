//! Typed source-scene presentation scheduling.
//!
//! The source renderer can finish a scene while display controls continue to
//! advance. A forced-blank release also retains the previously completed
//! scene for one presentation interval. This module models those observable
//! frames directly; it contains no machine address space or processor state.

use crate::shell::FrameSnapshot;

/// One completed scene paired with the live display state that presents it.
#[derive(Clone, Debug)]
pub struct SourcePresentation<T> {
    pub scene: FrameSnapshot,
    pub presentation: FrameSnapshot,
    pub content: T,
    /// An authored blank-release boundary must not interpolate the following
    /// scene's unrelated camera into the retained scene.
    pub snap_scene: bool,
}

impl<T> SourcePresentation<T> {
    pub fn frame(&self) -> FrameSnapshot {
        compose_source_presentation(&self.scene, &self.presentation)
    }
}

#[derive(Clone, Debug)]
pub struct CompletedPresentation<T> {
    pub value: T,
    pub snap_scene: bool,
}

/// Generic completed-frame retention used by both the native runtime and its
/// independent retail comparison adapter.
#[derive(Clone, Debug)]
pub struct CompletedPresentationQueue<T> {
    last_presented: Option<T>,
    queued: Option<CompletedPresentation<T>>,
    delayed: bool,
    snap_after_release: bool,
}

impl<T> Default for CompletedPresentationQueue<T> {
    fn default() -> Self {
        Self {
            last_presented: None,
            queued: None,
            delayed: false,
            snap_after_release: false,
        }
    }
}

impl<T: Clone> CompletedPresentationQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.last_presented = None;
        self.queued = None;
        self.delayed = false;
        self.snap_after_release = false;
    }

    pub fn advance(
        &mut self,
        candidate: T,
        releases_completed_scene: bool,
        retains_delayed_scene: bool,
    ) -> Option<CompletedPresentation<T>> {
        let (value, snap_scene) = if releases_completed_scene {
            self.delayed = true;
            self.snap_after_release = true;
            self.queued = Some(CompletedPresentation {
                value: candidate,
                snap_scene: false,
            });
            (
                self.last_presented
                    .clone()
                    .or_else(|| self.queued.as_ref().map(|queued| queued.value.clone()))?,
                false,
            )
        } else if self.delayed && retains_delayed_scene {
            let queued = self.queued.replace(CompletedPresentation {
                value: candidate,
                snap_scene: self.snap_after_release,
            })?;
            self.snap_after_release = false;
            (queued.value, queued.snap_scene)
        } else if self.delayed {
            // Once the authored transition ends, the source resumes its
            // normal completed-frame cadence and drops the obsolete queued
            // scene instead of carrying a permanent extra frame of latency.
            self.queued = None;
            self.delayed = false;
            (candidate, std::mem::take(&mut self.snap_after_release))
        } else {
            (candidate, false)
        };

        self.last_presented = Some(value.clone());
        Some(CompletedPresentation { value, snap_scene })
    }
}

/// Retains completed source scenes across a forced-blank release.
#[derive(Clone, Debug)]
pub struct SourcePresentationQueue<T> {
    previous: Option<(FrameSnapshot, T)>,
    completed: CompletedPresentationQueue<SourcePresentation<T>>,
}

impl<T> Default for SourcePresentationQueue<T> {
    fn default() -> Self {
        Self {
            previous: None,
            completed: CompletedPresentationQueue::default(),
        }
    }
}

impl<T: Clone> SourcePresentationQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.previous = None;
        self.completed.reset();
    }

    /// Accept the next completed simulation scene. The first scene seeds the
    /// queue; subsequent calls return the source scene visible for this
    /// presentation interval.
    pub fn advance(
        &mut self,
        current: FrameSnapshot,
        current_content: T,
    ) -> Option<SourcePresentation<T>> {
        let (scene, content) = self.previous.replace((current.clone(), current_content))?;
        let candidate = SourcePresentation {
            scene,
            presentation: current,
            content,
            snap_scene: false,
        };
        let releases_completed_scene = candidate.scene.display_forced_blank
            && !candidate.presentation.display_forced_blank
            && candidate.scene.screen_wipe.active;
        let retains_delayed_scene = candidate.presentation.windowmode != 0;

        let completed =
            self.completed
                .advance(candidate, releases_completed_scene, retains_delayed_scene)?;
        let mut presented = completed.value;
        presented.snap_scene = completed.snap_scene;
        Some(presented)
    }
}

/// Combine a completed source bitmap scene with the live display state used
/// to present it. Scene-owned data remains tied to the completed bitmap;
/// palette, scroll, windows, and display controls remain live.
pub fn compose_source_presentation(
    scene: &FrameSnapshot,
    presentation: &FrameSnapshot,
) -> FrameSnapshot {
    let mut aligned = presentation.clone();
    let presentation_palette = aligned.scene_style.game_palette;
    aligned.scene_style = scene.scene_style;
    aligned.scene_style.game_palette = presentation_palette;
    aligned.point_pixels.clone_from(&scene.point_pixels);
    aligned.meters = scene.meters;
    aligned.stayblack = scene.stayblack;
    aligned.gameflags = scene.gameflags;
    aligned.gameframe = scene.gameframe;
    aligned.display_black_subtraction = scene.display_black_subtraction;
    aligned.screen_wipe = scene.screen_wipe;
    aligned.boostcnt = scene.boostcnt;
    aligned.arrows = scene.arrows;
    aligned.player_view_mode = scene.player_view_mode;
    aligned.stage = scene.stage;
    aligned.stage_banner = scene.stage_banner;
    aligned.scramble_banner = scene.scramble_banner;
    aligned.shield_cur = scene.shield_cur;
    aligned.shield_max = scene.shield_max;
    aligned.boss_hp_cur = scene.boss_hp_cur;
    aligned.boss_hp_max = scene.boss_hp_max;
    aligned.lives = scene.lives;
    aligned.bombs = scene.bombs;
    aligned.specflash = scene.specflash;
    aligned.shieldup = scene.shieldup;
    aligned.msg_count1 = scene.msg_count1;
    aligned.msg_count2 = scene.msg_count2;
    aligned.radio_face_frame = scene.radio_face_frame;
    aligned.whichfriend = scene.whichfriend;
    aligned.friends_meter = scene.friends_meter;
    aligned.message_text.clone_from(&scene.message_text);
    aligned
        .radio_presentation
        .clone_from(&scene.radio_presentation);
    aligned
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::screen_wipe::{ScreenWipeKind, ScreenWipeState};

    fn frame(gameframe: u16, brightness: u8, forced_blank: bool) -> FrameSnapshot {
        FrameSnapshot {
            gameframe,
            display_brightness: brightness,
            display_forced_blank: forced_blank,
            screen_wipe: ScreenWipeState {
                kind: ScreenWipeKind::HorizontalReveal,
                frame: gameframe as u8,
                active: true,
            },
            windowmode: 1,
            ..FrameSnapshot::default()
        }
    }

    #[test]
    fn completed_scene_uses_live_display_controls() {
        let scene = frame(10, 0, true);
        let presentation = frame(11, 3, false);

        let composed = compose_source_presentation(&scene, &presentation);

        assert_eq!(composed.gameframe, scene.gameframe);
        assert_eq!(composed.screen_wipe, scene.screen_wipe);
        assert_eq!(composed.display_brightness, presentation.display_brightness);
        assert_eq!(
            composed.display_forced_blank,
            presentation.display_forced_blank
        );
    }

    #[test]
    fn blank_release_retains_then_advances_completed_scenes() {
        let mut queue = SourcePresentationQueue::new();
        assert!(queue.advance(frame(9, 0, true), 9).is_none());

        let before_release = queue
            .advance(frame(10, 0, true), 10)
            .expect("seeded presentation");
        assert_eq!(before_release.scene.gameframe, 9);

        let held = queue
            .advance(frame(11, 3, false), 11)
            .expect("held presentation");
        assert_eq!(held.scene.gameframe, 9);

        let released = queue
            .advance(frame(12, 6, false), 12)
            .expect("released presentation");
        assert_eq!(released.scene.gameframe, 10);
        assert_eq!(released.content, 10);
        assert!(!released.snap_scene);

        let cut = queue
            .advance(frame(13, 9, false), 13)
            .expect("first post-release scene");
        assert_eq!(cut.scene.gameframe, 11);
        assert_eq!(cut.content, 11);
        assert!(cut.snap_scene);

        let mut transition_complete = frame(14, 12, false);
        transition_complete.screen_wipe.active = false;
        transition_complete.windowmode = 0;
        let caught_up = queue
            .advance(transition_complete, 14)
            .expect("first scene after the transition");
        assert_eq!(caught_up.scene.gameframe, 13);
        assert_eq!(caught_up.content, 13);
        assert!(!caught_up.snap_scene);

        let following = queue
            .advance(frame(15, 15, false), 15)
            .expect("following scene");
        assert_eq!(following.scene.gameframe, 14);
        assert!(!following.snap_scene);
    }
}
