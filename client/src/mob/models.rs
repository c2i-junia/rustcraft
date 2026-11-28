//! How each kind of mob is drawn: its model, and the clips of the model that play its actions,
//! which `mobs.ron` gives (`MobProps::model`, `MobProps::clips`). The models are built by the
//! `scripts/generate_*.py` generators.

use std::collections::HashMap;

use bevy::prelude::*;
use shared::world::{Coat, MobAction, MobKind};

pub use shared::content::MobClip;

/// The scene a mob of `kind` is drawn with: a horse's is that of its coat, whose model has the
/// parts of `Horse.glb`, which the clips of the kind move (`scripts/generate_animals.py`).
pub fn scene_path(kind: MobKind, coat: Option<Coat>) -> String {
    match (kind, coat) {
        (MobKind::Horse, Some(coat)) if coat != Coat::Bay => {
            format!("models/animated/Horse{coat:?}.glb")
        }
        _ => kind.props().model.clone(),
    }
}

/// How far above the origin of the model of `kind` the bottom of its box is, in metres: the
/// model is drawn that much lower than its box (`MobProps::lift`).
pub fn model_lift(kind: MobKind) -> f32 {
    kind.props().lift
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
    let has = |clip| kind.props().clips.contains(&clip);
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
            let props = kind.props();
            let (graph, indices) =
                AnimationGraph::from_clips((0..props.clips.len()).map(|index| {
                    asset_server
                        .load(GltfAssetLabel::Animation(index).from_asset(props.model.as_str()))
                }));
            KindAnimations {
                graph: graphs.add(graph),
                nodes: props.clips.iter().copied().zip(indices).collect(),
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
                    kind.props().clips.contains(&clip),
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
            assert_eq!(
                kind.props().clips,
                [MobClip::Attack, MobClip::Idle, MobClip::Walk]
            );
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
