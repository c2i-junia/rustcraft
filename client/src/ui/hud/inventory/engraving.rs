//! The engraving table's section of the inventory panel, where the chest section goes: the slot
//! of the item, and beside it a button per enchantment the item takes, with its levels and what
//! the next one does and costs. The server makes it (`InventoryAction::Enchant`).

use crate::i18n::{tr, trf};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::{observe, Activate, Button as UiButton};
use shared::players::inventory::{InventoryAction, InventoryRefusal, ENGRAVING_SLOT};
use shared::players::{Inventory, Player};
use shared::world::{enchantment_cost, Enchantment, MAX_ENCHANTMENT_LEVEL};

use super::setup::icon;
use super::{EngravingRow, InventoryCell};
use crate::network::buffered_client::CurrentFrameInputs;
use crate::player::CurrentPlayerMarker;
use crate::ui::theme::{
    BEVEL_LO, BEVEL_WIDTH, EXPERIENCE, EXPERIENCE_HI, FONT_S, MUTED, SLOT, SPACE_L, SPACE_S,
    SPACE_XS, TEXT,
};
use crate::ui::widgets::button::{ButtonVariant, ThemedButton};
use crate::ui::widgets::focus::FocusRing;
use crate::ui::widgets::slot::slot;
use crate::ui::widgets::tooltip::Tooltip;
use crate::world::AtlasWrapper;

pub const ENGRAVING_TITLE: &str = "engraving.title";

/// The most enchantments an item takes: an axe's.
const MAX_CHOICES: usize = 3;
const ROW_HEIGHT: f32 = 36.0;
const PIP: f32 = 10.0;

/// The button of the enchantment of this index among those the item takes.
#[derive(Component, Clone, Copy)]
pub struct EnchantChoice(usize);

/// A text of a button.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum ChoiceText {
    Name,
    Effect,
    Cost,
}

/// The mark of this level of a button, lit once the item has it.
#[derive(Component, Clone, Copy)]
pub struct ChoicePip(u32);

/// The line under the buttons.
#[derive(Component)]
pub struct EngravingStatus;

/// The section, hidden until an engraving table is open.
pub(super) fn spawn_engraving_row(
    parent: &mut ChildSpawnerCommands,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
) {
    let text = |color: Color| {
        (
            Text::new(""),
            TextFont {
                font: font.clone().into(),
                font_size: px(FONT_S).into(),
                ..default()
            },
            TextColor(color),
            Pickable::IGNORE,
        )
    };
    let column = || Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(SPACE_XS),
        ..default()
    };
    parent
        .spawn((
            EngravingRow,
            Node {
                display: Display::None,
                align_items: AlignItems::Center,
                column_gap: px(SPACE_L),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                InventoryCell { id: ENGRAVING_SLOT },
                Tooltip::default(),
                slot(tr("engraving.slot"), None, icon(atlas), font.clone()),
            ));
            row.spawn(Node {
                flex_grow: 1.0,
                ..column()
            })
            .with_children(|choices| {
                for index in 0..MAX_CHOICES {
                    choices
                        .spawn((
                            EnchantChoice(index),
                            // the widget before the label: its `Role::Button` must win
                            UiButton,
                            AccessibleLabel::new(""),
                            TabIndex(-1),
                            Hovered::default(),
                            FocusRing,
                            ThemedButton(ButtonVariant::Default),
                            Node {
                                height: px(ROW_HEIGHT),
                                border: UiRect::all(px(BEVEL_WIDTH)),
                                padding: UiRect::horizontal(px(SPACE_S)),
                                align_items: AlignItems::Center,
                                column_gap: px(SPACE_S),
                                ..default()
                            },
                            Visibility::Hidden,
                            observe(choose_enchantment),
                        ))
                        .with_children(|choice| {
                            choice.spawn((
                                ChoiceText::Name,
                                text(TEXT),
                                Node {
                                    width: px(90),
                                    ..default()
                                },
                            ));
                            for level in 1..=MAX_ENCHANTMENT_LEVEL {
                                choice.spawn((
                                    ChoicePip(level),
                                    Node {
                                        width: px(PIP),
                                        height: px(PIP),
                                        border: UiRect::all(px(1)),
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                    BackgroundColor(SLOT),
                                    BorderColor::all(BEVEL_LO),
                                    UiTransform::from_rotation(Rot2::degrees(45.0)),
                                    Pickable::IGNORE,
                                ));
                            }
                            choice.spawn((
                                ChoiceText::Effect,
                                text(MUTED),
                                Node {
                                    flex_grow: 1.0,
                                    margin: UiRect::left(px(SPACE_S)),
                                    ..default()
                                },
                            ));
                            choice.spawn((ChoiceText::Cost, text(EXPERIENCE_HI)));
                        });
                }
                choices.spawn((EngravingStatus, text(MUTED)));
            });
        });
}

/// The enchantment of the button `index`, for the item in the slot.
fn choice(inventory: &Inventory, index: usize) -> Option<Enchantment> {
    inventory.open_engraver?;
    let stack = inventory.inner.get(&ENGRAVING_SLOT)?;
    stack.item_id.enchantments().nth(index)
}

/// Asks the server for the enchantment clicked.
fn choose_enchantment(
    event: On<Activate>,
    choices: Query<&EnchantChoice, Without<InteractionDisabled>>,
    inventory: Res<Inventory>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    let Ok(&EnchantChoice(index)) = choices.get(event.entity) else {
        return;
    };
    if let Some(enchantment) = choice(&inventory, index) {
        let action = InventoryAction::Enchant(enchantment);
        frame_inputs.0.inventory_actions.push(action);
    }
}

/// "1 level", "5 levels".
pub(super) fn levels(count: u32) -> String {
    match count {
        1 => tr("engraving.one_level").to_string(),
        count => trf("engraving.levels", &[("count", &count)]),
    }
}

/// Shows the enchantments the item in the slot takes, disabling those the player cannot have.
#[allow(clippy::type_complexity)]
pub fn display_engraving(
    inventory: Res<Inventory>,
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut buttons: Query<(
        Entity,
        &EnchantChoice,
        &mut TabIndex,
        &mut Visibility,
        &AccessibleLabel,
        Has<InteractionDisabled>,
        &Children,
    )>,
    mut texts: Query<(&ChoiceText, &mut Text)>,
    mut pips: Query<(&ChoicePip, &mut BackgroundColor)>,
    mut status: Query<&mut Text, (With<EngravingStatus>, Without<ChoiceText>)>,
    mut commands: Commands,
) {
    let player = player.single().ok();
    let creative = player.is_some_and(|player| player.game_mode.is_creative());
    let experience = player.map_or(0, |player| player.experience.level);
    let held = inventory.inner.get(&ENGRAVING_SLOT);
    let set = |text: &mut Text, shown: &str| {
        if text.0 != shown {
            text.0 = shown.to_string();
        }
    };
    if let Ok(mut text) = status.single_mut() {
        let shown = match held {
            None => tr("engraving.put_in").to_string(),
            Some(_) if creative => tr("engraving.free_creative").to_string(),
            Some(_) => trf("engraving.you_have", &[("levels", &levels(experience))]),
        };
        set(&mut text, &shown);
    }
    for (entity, &EnchantChoice(index), mut tab, mut visibility, label, disabled, children) in
        &mut buttons
    {
        let shown = held.zip(choice(&inventory, index));
        visibility.set_if_neq(match shown {
            Some(_) => Visibility::Inherited,
            None => Visibility::Hidden,
        });
        tab.set_if_neq(TabIndex(if shown.is_some() { 0 } else { -1 }));
        let Some((stack, enchantment)) = shown else {
            continue;
        };
        let level = stack.enchantment_level(enchantment);
        let next = level + 1;
        let full = level >= MAX_ENCHANTMENT_LEVEL;
        let cost = enchantment_cost(next);
        let refusal = if full {
            Some(InventoryRefusal::NothingToEnchant)
        } else if !creative && experience < cost {
            Some(InventoryRefusal::NotEnoughLevels(cost))
        } else {
            None
        };
        if refusal.is_some() != disabled {
            if disabled {
                commands.entity(entity).remove::<InteractionDisabled>();
            } else {
                commands.entity(entity).insert(InteractionDisabled);
            }
        }
        let price = match full {
            true => tr("engraving.complete").to_string(),
            false if creative => tr("engraving.free").to_string(),
            false => levels(cost),
        };
        let effect = crate::i18n::enchantment_effect(enchantment, if full { level } else { next });
        let name = crate::i18n::enchantment_title(enchantment, if full { level } else { next });
        let described = trf("engraving.choice", &[("name", &name), ("effect", &effect), ("price", &price)]);
        if label.0 != described {
            commands
                .entity(entity)
                .insert(AccessibleLabel::new(described));
        }
        for child in children.iter() {
            if let Ok((part, mut text)) = texts.get_mut(child) {
                match part {
                    ChoiceText::Name => set(&mut text, crate::i18n::enchantment_name(enchantment)),
                    ChoiceText::Effect => set(&mut text, &effect),
                    ChoiceText::Cost => set(&mut text, &price),
                }
            }
            if let Ok((&ChoicePip(mark), mut color)) = pips.get_mut(child) {
                let lit = if mark <= level { EXPERIENCE } else { SLOT };
                color.set_if_neq(BackgroundColor(lit));
            }
        }
    }
}
