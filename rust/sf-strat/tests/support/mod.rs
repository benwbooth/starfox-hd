use sf_game::alien::{Alien, ExplosionSize, ObjectVisualKind};

/// Stable trace identity for native visuals that have no drawable source mesh.
/// Abstract explosion headers stay typed so their small numeric header values
/// can never be mistaken for flat catalog mesh ids.
pub fn trace_visual_identity(object: &Alien) -> String {
    match object.visual_kind {
        ObjectVisualKind::ExplosionEnvelope(ExplosionSize::Small) => "explosion-small".into(),
        ObjectVisualKind::ExplosionEnvelope(ExplosionSize::Medium) => "explosion-medium".into(),
        ObjectVisualKind::ExplosionEnvelope(ExplosionSize::Large) => "explosion-large".into(),
        ObjectVisualKind::ExplosionEnvelope(ExplosionSize::Oversized) => {
            "explosion-oversized".into()
        }
        ObjectVisualKind::Mesh | ObjectVisualKind::ScaledSprite => object.shape.to_string(),
    }
}
