//! What the local player achieved in the world, as the server last told, and the toasts of what
//! it has just earned.

use bevy::prelude::*;
use shared::players::progress::{Achievement, Progress};

use crate::state::GameState;
use crate::ui::assets::load_font;
use crate::ui::widgets::toast::{show_toast, ToastKind};

/// Earned at once beyond this many, the achievements are counted in one toast.
const MOST_NAMED: usize = 2;

/// The progress of the local player, `None` until the server told it.
#[derive(Resource, Debug, Default)]
pub struct PlayerProgress(pub Option<Progress>);

/// What was earned and not shown yet: it may come while the world loads.
#[derive(Resource, Debug, Default)]
struct JustEarned(Vec<Achievement>);

/// The server told the progress of the local player, with what it has just earned.
pub struct Earn {
    pub progress: Progress,
    pub earned: Vec<Achievement>,
}

impl Command for Earn {
    type Out = ();

    fn apply(self, world: &mut World) {
        world.resource_mut::<PlayerProgress>().0 = Some(self.progress);
        world.resource_mut::<JustEarned>().0.extend(self.earned);
    }
}

pub fn progress_plugin(app: &mut App) {
    app.init_resource::<PlayerProgress>()
        .init_resource::<JustEarned>()
        .add_systems(Update, show_earned.run_if(in_state(GameState::Game)))
        .add_systems(
            OnExit(GameState::Game),
            |mut progress: ResMut<PlayerProgress>, mut earned: ResMut<JustEarned>| {
                progress.0 = None;
                earned.0.clear();
            },
        );
}

/// What the toasts of `earned` say.
fn toasts(earned: &[Achievement]) -> Vec<String> {
    if earned.len() > MOST_NAMED {
        return vec![format!("{} achievements earned", earned.len())];
    }
    let named = earned.iter();
    named
        .map(|achievement| format!("Achievement: {}", achievement.title()))
        .collect()
}

fn show_earned(mut earned: ResMut<JustEarned>, assets: Res<AssetServer>, mut commands: Commands) {
    if earned.0.is_empty() {
        return;
    }
    for text in toasts(&std::mem::take(&mut earned.0)) {
        let font = load_font(&assets);
        show_toast(&mut commands, &text, ToastKind::Achievement, font);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn many_achievements_earned_at_once_are_counted() {
        let earned = Achievement::ALL;
        assert_eq!(
            toasts(&earned[..1]),
            ["Achievement: Better Than Bare Hands"]
        );
        assert_eq!(toasts(&earned[..2]).len(), 2);
        assert_eq!(toasts(&earned[..5]), ["5 achievements earned"]);
    }
}
