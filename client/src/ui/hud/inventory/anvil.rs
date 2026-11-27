//! The anvil's section of the inventory panel, where the chest section goes: the slot of the
//! item worked and that of its material, the name to give it, what the work does and costs, and
//! the button that asks the server for it (`InventoryAction::Forge`).

use crate::i18n::{tr, trf};
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::{observe, Activate};
use shared::players::inventory::{anvil_slots, InventoryAction, ANVIL_LEFT_SLOT, ANVIL_RIGHT_SLOT};
use shared::players::{Inventory, Player};
use shared::world::{
    forge, repair_material, ForgeRefusal, ForgeWork, Ingredient, ItemName, ItemStack,
    MAX_ITEM_NAME_CHARS,
};

use super::engraving::levels;
use super::setup::icon;
use super::{item_name, AnvilRow, InventoryCell};
use crate::network::buffered_client::CurrentFrameInputs;
use crate::player::CurrentPlayerMarker;
use crate::ui::theme;
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::slot::slot;
use crate::ui::widgets::text_field::{set_text, spawn_text_field, Cancel, Submit, TextField};
use crate::ui::widgets::tooltip::Tooltip;
use crate::world::AtlasWrapper;

pub const ANVIL_TITLE: &str = "anvil.title";

/// The column the name field is put into, once the section is spawned.
#[derive(Component)]
pub struct AnvilNameHolder;

/// The input of the name field.
#[derive(Component)]
pub struct AnvilNameField;

/// The line telling what the work does, or why there is none.
#[derive(Component)]
pub struct AnvilWork;

/// The line telling what the work costs.
#[derive(Component)]
pub struct AnvilCost;

/// The button that asks for the work.
#[derive(Component)]
pub struct ForgeButton;

/// The section, hidden until an anvil is open.
pub(super) fn spawn_anvil_row(
    parent: &mut ChildSpawnerCommands,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
) {
    let text = |color: Color| {
        (
            Text::new(""),
            TextFont {
                font: font.clone().into(),
                font_size: px(theme::font_s()).into(),
                ..default()
            },
            TextColor(color),
            Pickable::IGNORE,
        )
    };
    parent
        .spawn((
            AnvilRow,
            Node {
                display: Display::None,
                align_items: AlignItems::Center,
                column_gap: px(theme::space_l()),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                InventoryCell {
                    id: ANVIL_LEFT_SLOT,
                },
                Tooltip::default(),
                slot(tr("anvil.item_slot"), None, icon(atlas), font.clone()),
            ));
            row.spawn((
                Text::new("+"),
                TextFont {
                    font: font.clone().into(),
                    font_size: px(theme::font_l()).into(),
                    ..default()
                },
                TextColor(theme::muted()),
                Pickable::IGNORE,
            ));
            row.spawn((
                InventoryCell {
                    id: ANVIL_RIGHT_SLOT,
                },
                Tooltip::default(),
                slot(tr("anvil.material_slot"), None, icon(atlas), font.clone()),
            ));
            row.spawn((
                AnvilNameHolder,
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme::space_s()),
                    min_width: px(220),
                    ..default()
                },
            ))
            .with_children(|column| {
                column.spawn((AnvilWork, text(theme::text())));
                column
                    .spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: px(theme::space_s()),
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn((
                            AnvilCost,
                            text(theme::experience_hi()),
                            Node {
                                flex_grow: 1.0,
                                ..default()
                            },
                        ));
                        line.spawn((
                            ForgeButton,
                            button(tr("anvil.forge"), ButtonVariant::Primary, font.clone()),
                            observe(forge_clicked),
                        ));
                    });
            });
        });
}

/// Puts the name field at the top of its column, once the section is spawned: a text field is
/// spawned by commands, outside the builder of the panel.
pub fn spawn_anvil_name_field(
    holders: Query<Entity, Added<AnvilNameHolder>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for holder in &holders {
        let font = crate::ui::assets::load_font(&asset_server);
        let field = spawn_text_field(
            &mut commands,
            &TextField {
                label: tr("anvil.name"),
                placeholder: tr("anvil.name"),
                max_characters: Some(MAX_ITEM_NAME_CHARS),
                font,
                font_size: theme::font_s(),
                reserve_error: false,
            },
        );
        commands.entity(holder).insert_children(0, &[field.root]);
        commands
            .entity(field.input)
            .insert(AnvilNameField)
            .observe(forge_submitted)
            .observe(|_: On<Cancel>, mut focus: ResMut<InputFocus>| focus.clear());
    }
}

/// The name typed in the field.
fn typed_name(fields: &Query<&EditableText, With<AnvilNameField>>) -> ItemName {
    fields
        .single()
        .map(|text| ItemName::new(&text.value().to_string()))
        .unwrap_or_default()
}

fn ask_forge(inventory: &Inventory, name: ItemName, frame_inputs: &mut CurrentFrameInputs) {
    if inventory.open_anvil.is_some() && inventory.inner.contains_key(&ANVIL_LEFT_SLOT) {
        frame_inputs
            .0
            .inventory_actions
            .push(InventoryAction::Forge(name));
    }
}

/// Asks the server for the work, with the name typed.
fn forge_clicked(
    _: On<Activate>,
    disabled: Query<(), (With<ForgeButton>, With<InteractionDisabled>)>,
    fields: Query<&EditableText, With<AnvilNameField>>,
    inventory: Res<Inventory>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    if disabled.is_empty() {
        ask_forge(&inventory, typed_name(&fields), &mut frame_inputs);
    }
}

/// Enter in the name field asks for the work too, and gives the focus back to the panel.
fn forge_submitted(
    submit: On<Submit>,
    inventory: Res<Inventory>,
    mut focus: ResMut<InputFocus>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
) {
    ask_forge(&inventory, ItemName::new(&submit.value), &mut frame_inputs);
    focus.clear();
}

/// The name of what repairs `item`, as the line of a wrong material writes it.
fn material_name(material: Ingredient) -> String {
    match material {
        Ingredient::Item(item) => item_name(item),
        Ingredient::Tag(tag) => crate::i18n::tag_name(tag).to_string(),
    }
}

/// Fills the name field with the name of the item put in the first slot, shows what the anvil
/// would do with its slots and the name typed, and what it costs, and enables the button while
/// the player can have it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn display_anvil(
    inventory: Res<Inventory>,
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut fields: Query<&mut EditableText, With<AnvilNameField>>,
    mut work: Query<(&mut Text, &mut TextColor), (With<AnvilWork>, Without<AnvilCost>)>,
    mut cost: Query<(&mut Text, &mut TextColor), (With<AnvilCost>, Without<AnvilWork>)>,
    button: Query<(Entity, Has<InteractionDisabled>), With<ForgeButton>>,
    mut last_left: Local<Option<ItemStack>>,
    mut commands: Commands,
) {
    if inventory.open_anvil.is_none() && !anvil_slots().any(|s| inventory.inner.contains_key(&s)) {
        *last_left = None;
        return;
    }
    let left = inventory.inner.get(&ANVIL_LEFT_SLOT).copied();
    let right = inventory.inner.get(&ANVIL_RIGHT_SLOT).copied();
    let Ok(mut field) = fields.single_mut() else {
        return;
    };
    // Another item, or the same one worked: the field shows its name
    if left != *last_left {
        let name = left.and_then(|stack| stack.custom_name().map(str::to_string));
        set_text(&mut field, &name.unwrap_or_default());
        *last_left = left;
    }
    let name = ItemName::new(&field.value().to_string());
    let player = player.single().ok();
    let creative = player.is_some_and(|player| player.game_mode.is_creative());
    let experience = player.map_or(0, |player| player.experience.level);
    let forging = match left {
        Some(left) => forge(&left, right.as_ref(), name),
        None => Err(ForgeRefusal::NothingToWork),
    };
    let (work_line, work_color) = match forging {
        Ok(forging) => {
            let result = forging.result;
            let durability = result.durability().unwrap_or(0);
            let worn = result.item_type.wear().unwrap_or(0);
            let numbers: [(&str, &dyn std::fmt::Display); 2] = [
                ("left", &(durability.saturating_sub(worn))),
                ("durability", &durability),
            ];
            let line = match forging.work {
                ForgeWork::Repair => trf("anvil.repair", &numbers),
                ForgeWork::Merge => trf("anvil.merge", &numbers),
                ForgeWork::Rename => tr("anvil.rename").to_string(),
            };
            (line, theme::text())
        }
        Err(ForgeRefusal::NothingToWork) => (tr("anvil.put_in").to_string(), theme::muted()),
        Err(ForgeRefusal::WrongMaterial) => {
            let material = left
                .and_then(|left| repair_material(left.item_id))
                .map(material_name)
                .unwrap_or_default();
            (
                trf("anvil.wrong_material", &[("material", &material)]),
                theme::muted(),
            )
        }
        Err(ForgeRefusal::Unchanged) => (tr("anvil.unchanged").to_string(), theme::muted()),
        Err(ForgeRefusal::TooExpensive(levels_needed)) => (
            trf("anvil.too_expensive", &[("levels", &levels(levels_needed))]),
            theme::danger(),
        ),
    };
    let (cost_line, cost_color, possible) = match forging {
        Ok(_) if creative => (
            tr("anvil.free_creative").to_string(),
            theme::experience_hi(),
            true,
        ),
        Ok(forging) if experience < forging.cost => (
            trf(
                "anvil.not_enough",
                &[("levels", &levels(forging.cost)), ("have", &experience)],
            ),
            theme::danger(),
            false,
        ),
        Ok(forging) => (
            trf("anvil.cost", &[("levels", &levels(forging.cost))]),
            theme::experience_hi(),
            true,
        ),
        Err(_) => (String::new(), theme::muted(), false),
    };
    if let Ok((mut text, mut color)) = work.single_mut() {
        if text.0 != work_line {
            text.0 = work_line;
        }
        color.set_if_neq(TextColor(work_color));
    }
    if let Ok((mut text, mut color)) = cost.single_mut() {
        if text.0 != cost_line {
            text.0 = cost_line;
        }
        color.set_if_neq(TextColor(cost_color));
    }
    if let Ok((entity, disabled)) = button.single() {
        if possible == disabled {
            if possible {
                commands.entity(entity).remove::<InteractionDisabled>();
            } else {
                commands.entity(entity).insert(InteractionDisabled);
            }
        }
    }
}
