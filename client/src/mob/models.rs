//! How each kind of mob is drawn: its model, and the clips of the model that play its actions.

use std::collections::HashMap;

use bevy::prelude::*;
use shared::world::{MobAction, MobKind};

/// What a mob looks like it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobClip {
    Idle,
    Walk,
    Run,
    Eat,
}

/// The model of a kind of mob.
pub struct MobModel {
    pub path: &'static str,
    /// The scale the model is drawn at: the fox is modelled in centimetres.
    pub scale: f32,
    /// The index in the model of the animation of each clip it has.
    clips: &'static [(MobClip, usize)],
}

pub const FOX_PATH: &str = "models/animated/Fox.glb";
/// Built by `scripts/generate_sheep.py`.
pub const SHEEP_PATH: &str = "models/animated/Sheep.glb";

pub fn model(kind: MobKind) -> MobModel {
    match kind {
        MobKind::Fox => MobModel {
            path: FOX_PATH,
            scale: 0.01,
            // Survey, Walk and Run
            clips: &[(MobClip::Idle, 0), (MobClip::Walk, 1), (MobClip::Run, 2)],
        },
        MobKind::Sheep => MobModel {
            path: SHEEP_PATH,
            scale: 1.0,
            // sorted by name when exported: Eat, Idle and Walk
            clips: &[(MobClip::Eat, 0), (MobClip::Idle, 1), (MobClip::Walk, 2)],
        },
    }
}

/// The clip a mob of `kind` doing `action` plays, among those its model has: one without a run
/// walks, one that cannot eat stands.
pub fn clip_for(kind: MobKind, action: MobAction) -> MobClip {
    let wanted = match action {
        MobAction::Walk => MobClip::Walk,
        MobAction::Flee => MobClip::Run,
        MobAction::Idle => MobClip::Idle,
        MobAction::Graze => MobClip::Eat,
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
    }
}
