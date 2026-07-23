//! Typed Star Fox 2 runtime.
//!
//! The native runtime deliberately has no byte-addressed state container.
//! Source-machine encodings are decoded at data boundaries and game systems
//! operate on the domain structs exported from this module.

mod game;
mod input;
mod object;
mod render;
mod results;
mod state;

mod astropolis_assault;
mod campaign_major_objectives;

pub use game::{Error, Game};
pub use input::{Button, Buttons, InputState};
pub use object::{
    Angle, Behavior, CollisionClass, Object, ObjectFlags, ObjectId, ObjectKind, ObjectStore,
    PathCursor, PathId, ShapeId, Vector3, WeaponKind, OBJECT_CAPACITY,
};
pub use render::{AnimationState, Camera, MaterialSetId, RenderFlags, RenderObject, Rotation};
pub use state::{
    AstropolisBranch, AstropolisCoreSpike, AstropolisEye, AstropolisEyes, AstropolisMissionState,
    AstropolisPhase, AudioOutput, AudioState, CampaignState, CarrierAssaultPhase,
    CarrierAssaultState, CarrierReactorPanel, ChargeSound, CorneriaDefensePhase,
    CorneriaDefenseState, Difficulty, EladardMissionState, EladardPhase, EndingPhase, EndingState,
    FlightControlStyle, GameMode, GameOverChoice, GameOverDestination, GameOverPhase, GameOverState,
    GameState, IntroPhase, MissionId, MissionPhase, MissionState, MissionVisit, Pilot,
    PilotCraftClass, PilotCraftProfile, PilotSelectionCursor, PilotSelectionPhase,
    PilotSelectionState, PlayerBlasterState, PlayerCraftForm,
    PlayerCraftTransformation, PlayerCraftTransformationDirection, PlayerDamageState,
    PlayerWalkerState, RandomState, ResultsChoice, ResultsPhase, ResultsState, Roster, SoundEvent,
    StrategicMapActor, StrategicMapActorKind, StrategicMapAppearance, StrategicMapPhase,
    StrategicMapState, StrategicMapTutorialPage, StrategicOpeningPage, StrategicOpeningState,
    TitaniaMissionState, TitaniaPhase, TitaniaReactorStatus, TitaniaSurfaceSwitchStatus,
    TitleMenuItem, TitlePage, TitleState, WalkerJumpMotion, WalkerJumpState, WalkerMotionProfile,
    SOUND_EVENT_CAPACITY, STRATEGIC_MAP_ACTOR_CAPACITY,
};
