//! Native Star Fox 2 game implementation.
//!
//! Shipping code is expressed as typed Rust state and systems. The optional
//! `oracle-bridge` feature exposes the old byte-exact compatibility host only
//! for differential verification; it is not part of [`Game`].

mod native;

pub use native::{
    Angle, AnimationState, AstropolisBranch, AstropolisCoreSpike, AstropolisEye, AstropolisEyes,
    AstropolisMissionState, AstropolisPhase, AudioOutput, AudioState, Behavior, Button, Buttons,
    Camera, CampaignState, CarrierAssaultPhase, CarrierAssaultState, CarrierReactorPanel,
    ChargeSound, CollisionClass, CorneriaDefensePhase, CorneriaDefenseState, Difficulty,
    EladardMissionState, EladardPhase, EndingPhase, EndingState, Error, FlightControlStyle, Game,
    GameMode, GameOverChoice, GameOverDestination, GameOverPhase, GameOverState, GameState,
    InputState, IntroPhase, MaterialSetId, MissionId, MissionPhase, MissionState, MissionVisit,
    Object, ObjectFlags, ObjectId, ObjectKind, ObjectStore, PathCursor, PathId, Pilot,
    PilotCraftClass, PilotCraftProfile, PilotSelectionCursor, PilotSelectionPhase,
    PilotSelectionState, PlayerBlasterState, PlayerCraftForm,
    PlayerCraftTransformation, PlayerCraftTransformationDirection, PlayerDamageState,
    PlayerWalkerState, RandomState, RenderFlags, RenderObject, ResultsChoice, ResultsPhase,
    ResultsState, Roster, Rotation, ShapeId, SoundEvent, StrategicMapActor, StrategicMapActorKind,
    StrategicMapAppearance, StrategicMapPhase, StrategicMapState, StrategicMapTutorialPage,
    StrategicOpeningPage, StrategicOpeningState, TitaniaMissionState, TitaniaPhase,
    TitaniaReactorStatus, TitaniaSurfaceSwitchStatus, TitleMenuItem, TitlePage, TitleState,
    Vector3, WalkerJumpMotion, WalkerJumpState, WalkerMotionProfile, WeaponKind, OBJECT_CAPACITY,
    SOUND_EVENT_CAPACITY, STRATEGIC_MAP_ACTOR_CAPACITY,
};

#[cfg(feature = "oracle-bridge")]
#[path = "cpu_bridge.rs"]
mod cpu_bridge;
#[cfg(feature = "oracle-bridge")]
#[path = "map_host.rs"]
mod map_host;
#[cfg(feature = "oracle-bridge")]
#[path = "memory.rs"]
pub mod memory;
#[cfg(feature = "oracle-bridge")]
#[path = "object.rs"]
pub mod object;
#[cfg(feature = "oracle-bridge")]
#[path = "path_host.rs"]
mod path_host;
#[cfg(feature = "oracle-bridge")]
#[path = "strategy.rs"]
mod strategy;

#[cfg(feature = "oracle-bridge")]
pub mod oracle_compat;
