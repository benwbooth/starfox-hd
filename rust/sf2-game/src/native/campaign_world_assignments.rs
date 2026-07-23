//! Generated semantic campaign-world assignments from the retail ROM.
//!
//! Source-machine addresses and selection ordinals remain in the generator.
//! Regenerate or verify with `uv run python
//! tools/sf2/generate_campaign_world_assignments.py [--check]`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CampaignWorld {
    Venom,
    Titania,
    Macbeth,
    Eladard,
    Meteor,
    Fortuna,
}

pub const CAMPAIGN_WORLD_COUNT: usize = 6;
pub const MAX_OCCUPIED_WORLD_COUNT: usize = 3;
pub(super) const NORMAL_OCCUPIED_WORLD_COUNT: usize = 2;
pub(super) const NORMAL_CAMPAIGN_ASSIGNMENT_COUNT: usize = 6;
pub(super) const THREE_WORLD_CAMPAIGN_ASSIGNMENT_COUNT: usize = 20;

impl CampaignWorld {
    pub const ALL: [Self; CAMPAIGN_WORLD_COUNT] = [
        Self::Venom,
        Self::Titania,
        Self::Macbeth,
        Self::Eladard,
        Self::Meteor,
        Self::Fortuna,
    ];
}

pub(super) const NORMAL_CAMPAIGN_WORLD_ASSIGNMENTS: [[CampaignWorld; NORMAL_OCCUPIED_WORLD_COUNT];
    NORMAL_CAMPAIGN_ASSIGNMENT_COUNT] = [
    [CampaignWorld::Titania, CampaignWorld::Venom],
    [CampaignWorld::Eladard, CampaignWorld::Venom],
    [CampaignWorld::Meteor, CampaignWorld::Venom],
    [CampaignWorld::Eladard, CampaignWorld::Titania],
    [CampaignWorld::Meteor, CampaignWorld::Titania],
    [CampaignWorld::Meteor, CampaignWorld::Eladard],
];

pub(super) const THREE_WORLD_CAMPAIGN_ASSIGNMENTS: [[CampaignWorld; MAX_OCCUPIED_WORLD_COUNT];
    THREE_WORLD_CAMPAIGN_ASSIGNMENT_COUNT] = [
    [
        CampaignWorld::Venom,
        CampaignWorld::Titania,
        CampaignWorld::Macbeth,
    ],
    [
        CampaignWorld::Venom,
        CampaignWorld::Titania,
        CampaignWorld::Eladard,
    ],
    [
        CampaignWorld::Venom,
        CampaignWorld::Titania,
        CampaignWorld::Meteor,
    ],
    [
        CampaignWorld::Fortuna,
        CampaignWorld::Titania,
        CampaignWorld::Venom,
    ],
    [
        CampaignWorld::Venom,
        CampaignWorld::Macbeth,
        CampaignWorld::Eladard,
    ],
    [
        CampaignWorld::Venom,
        CampaignWorld::Macbeth,
        CampaignWorld::Meteor,
    ],
    [
        CampaignWorld::Venom,
        CampaignWorld::Macbeth,
        CampaignWorld::Fortuna,
    ],
    [
        CampaignWorld::Venom,
        CampaignWorld::Eladard,
        CampaignWorld::Meteor,
    ],
    [
        CampaignWorld::Fortuna,
        CampaignWorld::Eladard,
        CampaignWorld::Venom,
    ],
    [
        CampaignWorld::Venom,
        CampaignWorld::Meteor,
        CampaignWorld::Fortuna,
    ],
    [
        CampaignWorld::Titania,
        CampaignWorld::Macbeth,
        CampaignWorld::Eladard,
    ],
    [
        CampaignWorld::Meteor,
        CampaignWorld::Macbeth,
        CampaignWorld::Titania,
    ],
    [
        CampaignWorld::Titania,
        CampaignWorld::Macbeth,
        CampaignWorld::Fortuna,
    ],
    [
        CampaignWorld::Titania,
        CampaignWorld::Eladard,
        CampaignWorld::Meteor,
    ],
    [
        CampaignWorld::Fortuna,
        CampaignWorld::Eladard,
        CampaignWorld::Titania,
    ],
    [
        CampaignWorld::Titania,
        CampaignWorld::Meteor,
        CampaignWorld::Fortuna,
    ],
    [
        CampaignWorld::Meteor,
        CampaignWorld::Eladard,
        CampaignWorld::Macbeth,
    ],
    [
        CampaignWorld::Fortuna,
        CampaignWorld::Eladard,
        CampaignWorld::Macbeth,
    ],
    [
        CampaignWorld::Macbeth,
        CampaignWorld::Meteor,
        CampaignWorld::Fortuna,
    ],
    [
        CampaignWorld::Eladard,
        CampaignWorld::Meteor,
        CampaignWorld::Fortuna,
    ],
];
