//! How each kind of mob is drawn: its model, and the clips of the model that play its actions.

use std::collections::HashMap;

use bevy::prelude::*;
use shared::world::{Coat, MobAction, MobKind};

/// What a mob looks like it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobClip {
    Idle,
    Walk,
    Run,
    Eat,
    /// A strike, played once.
    Attack,
}

impl MobClip {
    /// Whether the clip plays once rather than over and over.
    pub fn plays_once(self) -> bool {
        self == MobClip::Attack
    }
}

/// The model of a kind of mob.
pub struct MobModel {
    pub path: &'static str,
    /// The index in the model of the animation of each clip it has, which the exporter sorts by
    /// name.
    clips: &'static [(MobClip, usize)],
}

const GRAZER_CLIPS: &[(MobClip, usize)] = &[
    (MobClip::Eat, 0),
    (MobClip::Idle, 1),
    (MobClip::Run, 2),
    (MobClip::Walk, 3),
];
const HUNTER_CLIPS: &[(MobClip, usize)] =
    &[(MobClip::Idle, 0), (MobClip::Run, 1), (MobClip::Walk, 2)];
const SHEEP_CLIPS: &[(MobClip, usize)] =
    &[(MobClip::Eat, 0), (MobClip::Idle, 1), (MobClip::Walk, 2)];
/// The clips of `scripts/generate_hostiles.py`.
const HOSTILE_CLIPS: &[(MobClip, usize)] =
    &[(MobClip::Attack, 0), (MobClip::Idle, 1), (MobClip::Walk, 2)];

/// The clips of `scripts/generate_villagers.py`.
const VILLAGER_CLIPS: &[(MobClip, usize)] = &[(MobClip::Idle, 0), (MobClip::Walk, 1)];

/// The models are built by `scripts/generate_sheep.py`, `scripts/generate_animals.py`,
/// `scripts/generate_hostiles.py` and `scripts/generate_villagers.py`.
pub fn model(kind: MobKind) -> MobModel {
    let (path, clips) = match kind {
        MobKind::Sheep => ("models/animated/Sheep.glb", SHEEP_CLIPS),
        MobKind::Fox => ("models/animated/Fox.glb", HUNTER_CLIPS),
        MobKind::Wolf => ("models/animated/Wolf.glb", HUNTER_CLIPS),
        MobKind::Cat => ("models/animated/Cat.glb", HUNTER_CLIPS),
        MobKind::Cow => ("models/animated/Cow.glb", GRAZER_CLIPS),
        MobKind::Pig => ("models/animated/Pig.glb", GRAZER_CLIPS),
        MobKind::Chicken => ("models/animated/Chicken.glb", GRAZER_CLIPS),
        MobKind::Rabbit => ("models/animated/Rabbit.glb", GRAZER_CLIPS),
        MobKind::Horse => ("models/animated/Horse.glb", GRAZER_CLIPS),
        MobKind::Goat => ("models/animated/Goat.glb", GRAZER_CLIPS),
        MobKind::Gnawer => ("models/animated/Gnawer.glb", HOSTILE_CLIPS),
        MobKind::Bramble => ("models/animated/Bramble.glb", HOSTILE_CLIPS),
        MobKind::Roller => ("models/animated/Roller.glb", HOSTILE_CLIPS),
        MobKind::Stumpling => ("models/animated/Stumpling.glb", HOSTILE_CLIPS),
        MobKind::Drowner => ("models/animated/Drowner.glb", HOSTILE_CLIPS),
        MobKind::Cinderling => ("models/animated/Cinderling.glb", HOSTILE_CLIPS),
        // Its three clips are one spin
        MobKind::Ember => ("models/animated/Ember.glb", HOSTILE_CLIPS),
        MobKind::GustOwl => ("models/animated/GustOwl.glb", HOSTILE_CLIPS),
        MobKind::EmberMother => ("models/animated/EmberMother.glb", HOSTILE_CLIPS),
        // Its three clips stand still
        MobKind::Arrow => ("models/animated/Arrow.glb", HOSTILE_CLIPS),
        MobKind::Farmer => ("models/animated/Farmer.glb", VILLAGER_CLIPS),
        MobKind::Smith => ("models/animated/Smith.glb", VILLAGER_CLIPS),
        MobKind::Cartographer => ("models/animated/Cartographer.glb", VILLAGER_CLIPS),
        // `scripts/generate_boat.py`: its three clips stand still
        MobKind::Boat => ("models/animated/Boat.glb", HOSTILE_CLIPS),
        // `scripts/generate_bobber.py`: it bobs, and plunges at a bite
        MobKind::Bobber => ("models/animated/Bobber.glb", HOSTILE_CLIPS),
        // `scripts/generate_caverns.py`
        MobKind::Glintfin => ("models/animated/Glintfin.glb", HOSTILE_CLIPS),
        MobKind::Shoalback => ("models/animated/Shoalback.glb", HOSTILE_CLIPS),
        MobKind::Mossback => ("models/animated/Mossback.glb", HOSTILE_CLIPS),
        MobKind::Shardling => ("models/animated/Shardling.glb", HOSTILE_CLIPS),
        MobKind::Slagcrawler => ("models/animated/Slagcrawler.glb", HOSTILE_CLIPS),
        MobKind::Donkey => ("models/animated/Donkey.glb", GRAZER_CLIPS),
        MobKind::Mule => ("models/animated/Mule.glb", GRAZER_CLIPS),
    };
    MobModel { path, clips }
}

/// The scene a mob of `kind` is drawn with: a horse's is that of its coat, whose model has the
/// parts of `Horse.glb`, which the clips of the kind move (`scripts/generate_animals.py`).
pub fn scene_path(kind: MobKind, coat: Option<Coat>) -> String {
    match (kind, coat) {
        (MobKind::Horse, Some(coat)) if coat != Coat::Bay => {
            format!("models/animated/Horse{coat:?}.glb")
        }
        _ => model(kind).path.to_string(),
    }
}

/// How far above the origin of the model of `kind` the bottom of its box is, in metres: the
/// model is drawn that much lower than its box. The Gust Owl's talons are 7 voxels up.
pub fn model_lift(kind: MobKind) -> f32 {
    match kind {
        MobKind::GustOwl => 7.0 / 16.0,
        _ => 0.0,
    }
}

/// The clip a mob of `kind` doing `action` plays, among those its model has: one without a run
/// walks, one that cannot eat or strike stands. A chase is a run, a walk for those without one
/// (the Roller's roll too): a wolf runs at its prey.
pub fn clip_for(kind: MobKind, action: MobAction) -> MobClip {
    let wanted = match action {
        MobAction::Walk => MobClip::Walk,
        MobAction::Flee => MobClip::Run,
        MobAction::Idle => MobClip::Idle,
        MobAction::Graze => MobClip::Eat,
        MobAction::Chase => MobClip::Run,
        MobAction::Attack => MobClip::Attack,
        // It sinks standing, the ground hiding it
        MobAction::Burrow => MobClip::Idle,
    };
    let has = |clip| model(kind).clips.iter().any(|&(c, _)| c == clip);
    match wanted {
        clip if has(clip) => clip,
        MobClip::Run => MobClip::Walk,
        _ => MobClip::Idle,
    }
}

/// The animation graph of each kind of mob, built when the first of its kind appears.
#[derive(Resource, Default)]
pub struct MobAnimations(HashMap<MobKind, KindAnimations>);

pub struct KindAnimations {
    pub graph: Handle<AnimationGraph>,
    pub nodes: HashMap<MobClip, AnimationNodeIndex>,
}

impl MobAnimations {
    pub fn of(
        &mut self,
        kind: MobKind,
        asset_server: &AssetServer,
        graphs: &mut Assets<AnimationGraph>,
    ) -> &KindAnimations {
        self.0.entry(kind).or_insert_with(|| {
            let model = model(kind);
            let (graph, indices) = AnimationGraph::from_clips(model.clips.iter().map(|&(_, i)| {
                asset_server.load(GltfAssetLabel::Animation(i).from_asset(model.path))
            }));
            KindAnimations {
                graph: graphs.add(graph),
                nodes: model
                    .clips
                    .iter()
                    .map(|&(clip, _)| clip)
                    .zip(indices)
                    .collect(),
            }
        })
    }

    /// The graph of `kind`, once a mob of it appeared.
    pub fn get(&self, kind: MobKind) -> Option<&KindAnimations> {
        self.0.get(&kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_plays_a_clip_the_model_has() {
        for kind in MobKind::ALL {
            for action in [
                MobAction::Walk,
                MobAction::Flee,
                MobAction::Idle,
                MobAction::Graze,
                MobAction::Chase,
                MobAction::Attack,
                MobAction::Burrow,
            ] {
                let clip = clip_for(kind, action);
                assert!(
                    model(kind).clips.iter().any(|&(c, _)| c == clip),
                    "{kind:?} {action:?}: {clip:?}"
                );
            }
        }
        assert_eq!(clip_for(MobKind::Sheep, MobAction::Graze), MobClip::Eat);
        assert_eq!(clip_for(MobKind::Sheep, MobAction::Flee), MobClip::Walk);
        assert_eq!(clip_for(MobKind::Fox, MobAction::Flee), MobClip::Run);
        assert_eq!(clip_for(MobKind::Fox, MobAction::Graze), MobClip::Idle);
        assert_eq!(clip_for(MobKind::Fox, MobAction::Attack), MobClip::Idle);
        for kind in [
            MobKind::Gnawer,
            MobKind::Bramble,
            MobKind::Roller,
            MobKind::Stumpling,
            MobKind::Drowner,
            MobKind::Cinderling,
            MobKind::Ember,
            MobKind::GustOwl,
            MobKind::Arrow,
        ] {
            assert_eq!(clip_for(kind, MobAction::Chase), MobClip::Walk);
            assert_eq!(clip_for(kind, MobAction::Attack), MobClip::Attack);
            assert_eq!(clip_for(kind, MobAction::Idle), MobClip::Idle);
            assert_eq!(model(kind).clips, HOSTILE_CLIPS);
        }
        assert_eq!(
            clip_for(MobKind::Stumpling, MobAction::Burrow),
            MobClip::Idle
        );
        assert!(MobClip::Attack.plays_once());
    }

    #[test]
    fn the_gust_owl_is_drawn_with_its_talons_at_the_bottom_of_its_box() {
        assert_eq!(model_lift(MobKind::GustOwl), 0.4375);
        for kind in MobKind::ALL.into_iter().filter(|&k| k != MobKind::GustOwl) {
            assert_eq!(model_lift(kind), 0.0, "{kind:?}");
        }
        assert!(!MobClip::Walk.plays_once());
    }
}
