//! The experience of the local player in survival: a bar right over the hotbar, filled as the
//! next level nears, and the level over its middle, between the hearts and the drumsticks.

use crate::i18n::{tr, trf};
use bevy::prelude::*;
use shared::players::experience::Experience;
use shared::players::Player;

use crate::player::CurrentPlayerMarker;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::InventoryRoot;
use crate::ui::theme::{
    BEVEL_LO, BEVEL_WIDTH, EXPERIENCE, EXPERIENCE_HI, EXPERIENCE_LO, FONT_M, SLOT, SPACE_XS,
};

/// The height of the bar, in logical px: its border and 4 px of fill.
const BAR_HEIGHT: f32 = 2.0 * BEVEL_WIDTH + 4.0;

/// The height the bar takes above the hotbar, in logical px: its margin and itself. The hearts
/// and the drumsticks sit on top of it.
pub const EXPERIENCE_BAND: f32 = SPACE_XS + BAR_HEIGHT;

/// The bar, shown in survival only.
#[derive(Component)]
pub struct ExperienceBar;

#[derive(Component)]
pub struct ExperienceFill;

/// The number over the bar, none at level 0.
#[derive(Component)]
pub struct LevelText;

/// The bar as the accessibility tree and the tests read it.
pub fn experience_label(experience: &Experience) -> String {
    let percent = (experience.progress() * 100.0) as u32;
    trf("hud.experience.label", &[("level", &experience.level), ("percent", &percent)])
}

/// Spawns the bar over the hotbar, hidden until the local player is known to be in survival.
pub fn spawn_experience_bar(
    mut commands: Commands,
    hotbar: Query<Entity, With<Hotbar>>,
    assets: Res<AssetServer>,
) {
    let Ok(hotbar) = hotbar.single() else {
        return;
    };
    commands.spawn((
        ExperienceBar,
        Name::new(tr("hud.experience")),
        AccessibleLabel::new(experience_label(&Experience::default())),
        Node {
            position_type: PositionType::Absolute,
            left: px(0.),
            right: px(0.),
            bottom: percent(100.),
            height: px(BAR_HEIGHT),
            margin: UiRect::bottom(px(SPACE_XS)),
            border: UiRect::all(px(BEVEL_WIDTH)),
            ..default()
        },
        BackgroundColor(SLOT),
        BorderColor::all(BEVEL_LO),
        Visibility::Hidden,
        Pickable::IGNORE,
        ChildOf(hotbar),
        children![
            (
                ExperienceFill,
                Node {
                    width: percent(0.),
                    height: percent(100.),
                    // A lit top edge and a shaded bottom one
                    border: UiRect::vertical(px(1.)),
                    ..default()
                },
                BackgroundColor(EXPERIENCE),
                BorderColor {
                    top: EXPERIENCE_HI,
                    bottom: EXPERIENCE_LO,
                    ..default()
                },
                Pickable::IGNORE,
            ),
            (
                LevelText,
                Text::new(""),
                TextFont {
                    font: assets.load(crate::ui::assets::FONT_PATH).into(),
                    font_size: px(FONT_M).into(),
                    ..default()
                },
                TextColor(EXPERIENCE_HI),
                TextLayout::justify(Justify::Center),
                TextShadow {
                    offset: Vec2::splat(2.0),
                    color: BEVEL_LO,
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.),
                    right: px(0.),
                    bottom: percent(100.),
                    ..default()
                },
                Pickable::IGNORE,
            )
        ],
    ));
}

/// Fills the bar with the experience of the local player, and shows it as the hearts: in
/// survival only, while the inventory is closed. Writes nothing while none of these changes.
pub fn update_experience_bar(
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut bar: Query<(Entity, &mut Visibility), With<ExperienceBar>>,
    mut fill: Query<&mut Node, With<ExperienceFill>>,
    mut level: Query<&mut Text, With<LevelText>>,
    panel: Query<&Visibility, (With<InventoryRoot>, Without<ExperienceBar>)>,
    mut shown: Local<Option<(Entity, Experience, bool)>>,
    mut commands: Commands,
) {
    let (Ok(player), Ok((bar, mut visibility))) = (player.single(), bar.single_mut()) else {
        return;
    };
    let open = panel.single().is_ok_and(|v| *v == Visibility::Visible);
    let is_shown = player.game_mode.takes_damage() && !open;
    // The bar of another session starts afresh
    let state = (bar, player.experience, is_shown);
    if *shown == Some(state) {
        return;
    }
    *shown = Some(state);

    *visibility = if is_shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let experience = player.experience;
    commands
        .entity(bar)
        .insert(AccessibleLabel::new(experience_label(&experience)));
    if let Ok(mut fill) = fill.single_mut() {
        fill.width = percent(experience.progress() * 100.0);
    }
    if let Ok(mut level) = level.single_mut() {
        level.0 = match experience.level {
            0 => String::new(),
            level => level.to_string(),
        };
    }
}
