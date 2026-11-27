//! The progress of the player in the world, over the pause menu: its achievements, earned
//! or not, and its statistics.

use crate::i18n::{tr, trf};
use accesskit::{NodeId, Role};
use bevy::{
    a11y::AccessibilityNode,
    ecs::entity::Entities,
    input_focus::{tab_navigation::TabGroup, AutoFocus, FocusCause, InputFocus, InputFocusVisible},
    prelude::*,
    ui_widgets::{observe, Activate},
};
use shared::players::progress::{Achievement, Stats};

use crate::state::GameState;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::hud::progress::PlayerProgress;
use crate::ui::hud::UiDialog;
use crate::ui::menus::modal_open;
use crate::ui::theme;
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::focus::keep_focus_inside;
use crate::ui::widgets::hints::{hint_bar, Hint, HINT_BAR_HEIGHT};
use crate::ui::widgets::list::{spawn_list_card, spawn_scroll_list, WheelStep};
use crate::ui::widgets::slot::ICON_SIZE;
use crate::ui::widgets::tabs::{spawn_tabs, TabButton, TabChange, Tabs};
use crate::ui::widgets::{raised_bevel, sunken_bevel};
use crate::world::MaterialResource;

pub const PROGRESS_TITLE: &str = "progress.title";

const HINTS: [Hint; 4] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "←→",
        action: "Tabs",
    },
    Hint {
        keys: "↑↓",
        action: "Scroll",
    },
    Hint {
        keys: "Esc",
        action: "Back",
    },
];

/// Those of the settings.
const PANEL_WIDTH: f32 = 720.0;
const PANEL_HEIGHT: f32 = 620.0;
/// How opaque the icon of an achievement not earned is.
const UNEARNED_ALPHA: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProgressTab {
    Achievements,
    Statistics,
}

impl ProgressTab {
    pub const ALL: [ProgressTab; 2] = [ProgressTab::Achievements, ProgressTab::Statistics];

    pub fn label(self) -> &'static str {
        match self {
            ProgressTab::Achievements => tr("progress.achievements"),
            ProgressTab::Statistics => tr("progress.statistics"),
        }
    }
}

/// The root of the screen.
#[derive(Component)]
pub struct ProgressRoot {
    /// The button that opened it, which gets the focus back.
    opener: Entity,
}

/// The content of the current tab, spawned again when the player chooses another one, or when
/// the progress changes.
#[derive(Component)]
struct ProgressPanel {
    tab: ProgressTab,
}

/// A request to close the screen, on its root, applied in `PostUpdate`, as `CloseSettings`.
#[derive(Component)]
struct CloseProgress;

/// Opens the screen, from the button `opener`.
pub struct OpenProgress {
    pub opener: Entity,
}

impl Command for OpenProgress {
    type Out = ();

    fn apply(self, world: &mut World) {
        let assets = world.resource::<AssetServer>().clone();
        spawn_progress(&mut world.commands(), &assets, self.opener);
        world.flush();
    }
}

pub fn progress_screen_plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            close_on_escape.run_if(not(modal_open)),
            (show_again, show_stats).run_if(resource_changed::<PlayerProgress>),
        )
            .run_if(any_with_component::<ProgressRoot>),
    )
    .add_systems(PostUpdate, close_progress);
}

fn text(content: impl Into<String>, size: f32, color: Color, font: &Handle<Font>) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font: font.clone().into(),
            font_size: px(size).into(),
            ..default()
        },
        TextColor(color),
        Pickable::IGNORE,
    )
}

fn spawn_progress(commands: &mut Commands, assets: &AssetServer, opener: Entity) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let root = commands
        .spawn((
            ProgressRoot { opener },
            UiDialog,
            AccessibilityNode(accesskit::Node::new(Role::Dialog)),
            AccessibleLabel::new(crate::i18n::tr(PROGRESS_TITLE)),
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    top: px(theme::space_xl()),
                    bottom: px(HINT_BAR_HEIGHT + theme::space_xl()),
                    left: px(theme::space_l()),
                    right: px(theme::space_l()),
                },
                ..default()
            },
            BackgroundColor(theme::bevel_lo().with_alpha(0.7)),
            Visibility::Visible,
            // over the pause menu (5)
            GlobalZIndex(6),
            TabGroup::modal(),
            DespawnOnExit(GameState::Game),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                width: px(PANEL_WIDTH),
                max_width: percent(100),
                height: px(PANEL_HEIGHT),
                max_height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::space_m()),
                padding: UiRect::all(px(theme::space_l())),
                border: UiRect::all(px(theme::bevel_width())),
                ..default()
            },
            BackgroundColor(theme::panel()),
            raised_bevel(),
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        text(
            crate::i18n::tr(PROGRESS_TITLE),
            theme::font_xl(),
            theme::text(),
            &font,
        ),
        ChildOf(panel),
    ));
    let labels = ProgressTab::ALL.map(ProgressTab::label);
    let tabs = Tabs {
        label: crate::i18n::tr(PROGRESS_TITLE),
        tabs: &labels,
        selected: 0,
    };
    let bar = spawn_tabs(commands, &tabs, font.clone());
    commands
        .entity(bar)
        .insert((AutoFocus, ChildOf(panel)))
        .observe(show_tab);
    commands.spawn((
        ProgressPanel {
            tab: ProgressTab::ALL[0],
        },
        AccessibilityNode(accesskit::Node::new(Role::TabPanel)),
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: px(0),
            row_gap: px(theme::space_m()),
            ..default()
        },
        ChildOf(panel),
    ));
    commands.spawn((
        Node::default(),
        ChildOf(panel),
        children![(
            button(tr("common.back"), ButtonVariant::Default, font.clone()),
            observe(move |_: On<Activate>, mut commands: Commands| {
                commands.entity(root).insert(CloseProgress);
            }),
        )],
    ));
    commands.spawn((
        hint_bar(&HINTS, font, assets.load(MONO_FONT_PATH)),
        ChildOf(root),
    ));
    keep_focus_inside(commands, root, Some(bar));
    commands.trigger(TabChange {
        entity: bar,
        index: 0,
    });
}

/// Shows the chosen tab, which labels its panel.
fn show_tab(
    change: On<TabChange>,
    panel: Single<(Entity, &mut ProgressPanel, &mut AccessibilityNode)>,
    tabs: Query<(Entity, &TabButton, &ChildOf)>,
    progress: Res<PlayerProgress>,
    materials: Res<MaterialResource>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let (entity, mut panel, mut node) = panel.into_inner();
    panel.tab = ProgressTab::ALL[change.index];
    node.set_label(panel.tab.label());
    if let Some((button, ..)) = tabs.iter().find(|(_, button, child_of)| {
        button.0 == change.index && child_of.parent() == change.entity
    }) {
        node.set_labelled_by(vec![NodeId(button.to_bits())]);
    }
    fill(
        &mut commands,
        entity,
        panel.tab,
        &progress,
        &materials,
        &assets,
    );
}

/// Shows the current tab again once an achievement was earned.
fn show_again(
    panels: Query<(Entity, &ProgressPanel)>,
    progress: Res<PlayerProgress>,
    materials: Res<MaterialResource>,
    assets: Res<AssetServer>,
    mut shown: Local<usize>,
    mut commands: Commands,
) {
    let earned = progress.0.as_ref().map_or(0, |p| p.achievements.len());
    if std::mem::replace(&mut *shown, earned) == earned {
        return;
    }
    for (entity, panel) in &panels {
        fill(
            &mut commands,
            entity,
            panel.tab,
            &progress,
            &materials,
            &assets,
        );
    }
}

fn fill(
    commands: &mut Commands,
    panel: Entity,
    tab: ProgressTab,
    progress: &PlayerProgress,
    materials: &MaterialResource,
    assets: &AssetServer,
) {
    commands.entity(panel).despawn_related::<Children>();
    let font: Handle<Font> = assets.load(FONT_PATH);
    match tab {
        ProgressTab::Achievements => {
            spawn_achievements(commands, panel, progress, materials, &font)
        }
        ProgressTab::Statistics => spawn_statistics(commands, panel, progress, &font),
    }
}

/// The achievements, the earned ones bright, under how many are.
fn spawn_achievements(
    commands: &mut Commands,
    panel: Entity,
    progress: &PlayerProgress,
    materials: &MaterialResource,
    font: &Handle<Font>,
) {
    let has = |achievement| progress.0.as_ref().is_some_and(|p| p.has(achievement));
    let earned = Achievement::ALL.into_iter().filter(|&a| has(a)).count();
    let count = trf(
        "progress.count",
        &[("earned", &earned), ("total", &Achievement::ALL.len())],
    );
    commands.spawn((
        text(count, theme::font_s(), theme::muted(), font),
        ChildOf(panel),
    ));
    let parts = spawn_scroll_list(commands, WheelStep::Item);
    commands.entity(parts.frame).insert(ChildOf(panel));
    commands.entity(parts.list).insert((
        AccessibilityNode(accesskit::Node::new(Role::List)),
        AccessibleLabel::new(ProgressTab::Achievements.label()),
    ));
    for achievement in Achievement::ALL {
        let has = has(achievement);
        let (title, goal) = (
            crate::i18n::achievement_title(achievement),
            crate::i18n::achievement_goal(achievement),
        );
        let (state, title_color) = if has {
            (tr("progress.earned"), theme::text())
        } else {
            (tr("progress.not_yet"), theme::muted())
        };
        let label = trf(
            "progress.card",
            &[("title", &title), ("state", &state), ("goal", &goal)],
        );
        let card = spawn_list_card(commands, parts.list, &label);
        let icon = commands
            .spawn((
                Node {
                    width: px(ICON_SIZE),
                    height: px(ICON_SIZE),
                    flex_shrink: 0.0,
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(card),
            ))
            .id();
        if let Some(atlas) = &materials.items {
            if let Some(sprite) = atlas.sprite(&format!("{:?}", achievement.icon())) {
                let mut image = ImageNode::from_atlas_image(atlas.texture.clone(), sprite);
                if !has {
                    image.color = Color::WHITE.with_alpha(UNEARNED_ALPHA);
                }
                commands.entity(icon).insert(image);
            }
        }
        commands.entity(card).with_children(|card| {
            card.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    min_width: px(0),
                    row_gap: px(theme::space_xs()),
                    ..default()
                },
                Pickable::IGNORE,
                children![
                    text(title, theme::font_m(), title_color, font),
                    text(goal, theme::font_s(), theme::muted(), font)
                ],
            ));
            let color = if has { theme::accent() } else { theme::muted() };
            card.spawn((
                text(state, theme::font_s(), color, font),
                TextLayout::no_wrap(),
            ));
        });
    }
}

/// The value of a statistic, by its place in `Stats::LABELS`.
#[derive(Component)]
pub struct StatValue(pub usize);

/// What the statistics read.
pub fn stat_values(progress: &PlayerProgress) -> [String; Stats::LABELS.len()] {
    progress
        .0
        .as_ref()
        .map(|progress| progress.stats)
        .unwrap_or_default()
        .values()
}

/// The statistics, a row each.
fn spawn_statistics(
    commands: &mut Commands,
    panel: Entity,
    progress: &PlayerProgress,
    font: &Handle<Font>,
) {
    for (i, value) in stat_values(progress).into_iter().enumerate() {
        commands.spawn((
            Node {
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(px(theme::space_m()), px(theme::space_s())),
                border: UiRect::all(px(theme::bevel_width())),
                ..default()
            },
            BackgroundColor(theme::slot()),
            sunken_bevel(),
            ChildOf(panel),
            children![
                text(
                    crate::i18n::tr_as("stat", Stats::LABELS[i]),
                    theme::font_m(),
                    theme::muted(),
                    font
                ),
                (
                    StatValue(i),
                    text(value, theme::font_m(), theme::text(), font)
                )
            ],
        ));
    }
}

/// Keeps the statistics shown up to date.
fn show_stats(progress: Res<PlayerProgress>, mut values: Query<(&StatValue, &mut Text)>) {
    let read = stat_values(&progress);
    for (stat, mut text) in &mut values {
        if text.0 != read[stat.0] {
            text.0.clone_from(&read[stat.0]);
        }
    }
}

fn close_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    roots: Query<Entity, With<ProgressRoot>>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for root in &roots {
            commands.entity(root).insert(CloseProgress);
        }
    }
}

/// Closes the screen back to the pause menu, the focus on the button that opened it, with its
/// ring when a key closed it.
fn close_progress(
    roots: Query<(Entity, &ProgressRoot), With<CloseProgress>>,
    entities: &Entities,
    keys: Res<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut visible: ResMut<InputFocusVisible>,
    mut commands: Commands,
) {
    for (root, screen) in &roots {
        commands.entity(root).despawn();
        if entities.contains(screen.opener) {
            focus.set(screen.opener, FocusCause::Navigated);
            visible.0 |= keys.get_just_pressed().next().is_some();
        } else {
            focus.clear();
        }
    }
}
