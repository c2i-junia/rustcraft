//! The trade section of the inventory panel, where the chest section goes: the offers of the
//! villager the player trades with, each a button. The server opens it (`TradeUpdate`), and
//! closes it when the villager is too far; closing the panel ends the trade.

use crate::i18n::{tr, trf};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::{observe, Activate, Button as UiButton};
use bevy_renet::RenetClient;
use shared::messages::ClientToServerMessage;
use shared::players::Inventory;
use shared::world::trade::{offers, Goods, TradeRefusal, MAX_OFFERS};
use shared::world::{ItemId, MobId, TradeView};

use super::setup::icon;
use super::{item_name, InventoryRoot, TradeGrid};
use crate::network::SendGameMessageExtension;
use crate::ui::hud::UIMode;
use crate::ui::theme;
use crate::ui::widgets::button::{ButtonVariant, ThemedButton};
use crate::ui::widgets::focus::FocusRing;
use crate::ui::widgets::slot::{
    count_label, slot_count, slot_icon, slot_node, SlotCount, SlotIcon, ICON_SIZE,
};
use crate::ui::widgets::tooltip::Tooltip;
use crate::world::{AtlasWrapper, MaterialResource};

/// The sprite of the directions a cartographer sells.
const DIRECTIONS_PATH: &str = "./graphics/hud/directions.png";
const DIRECTIONS_TITLE: &str = "trade.directions";

/// What the server said of the trade of the player.
#[derive(Message, Debug, Clone)]
pub struct TradeUpdated(pub Option<TradeView>);

/// The trade shown, if any.
#[derive(Resource, Default, Debug)]
pub struct OpenTrade(pub Option<TradeView>);

/// The button of the offer of this index.
#[derive(Component, Clone, Copy)]
pub struct TradeOffer(usize);

/// How many times the offer is still made.
#[derive(Component)]
pub struct TradeLeft;

/// What an offer asks, or gives.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum TradeSide {
    Cost,
    Goods,
}

#[derive(Component)]
pub struct DirectionsIcon;

/// The offers, hidden until a trade is open.
pub(super) fn spawn_trade_grid(
    parent: &mut ChildSpawnerCommands,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
    assets: &AssetServer,
) {
    let text = |text: &str| {
        (
            Text::new(text),
            TextFont {
                font: font.clone().into(),
                font_size: px(theme::font_s()).into(),
                ..default()
            },
            TextColor(theme::muted()),
            Pickable::IGNORE,
        )
    };
    parent
        .spawn((
            TradeGrid,
            Node {
                display: Display::None,
                grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
                row_gap: px(theme::space_xs()),
                column_gap: px(theme::space_xs()),
                ..default()
            },
        ))
        .with_children(|grid| {
            for index in 0..MAX_OFFERS {
                grid.spawn((
                    TradeOffer(index),
                    // the widget before the label: its `Role::Button` must win
                    UiButton,
                    AccessibleLabel::new(""),
                    TabIndex(-1),
                    Hovered::default(),
                    FocusRing,
                    ThemedButton(ButtonVariant::Default),
                    Tooltip::default(),
                    Node {
                        border: UiRect::all(px(theme::bevel_width())),
                        padding: UiRect::axes(px(theme::space_s()), px(theme::space_xs())),
                        align_items: AlignItems::Center,
                        column_gap: px(theme::space_s()),
                        ..default()
                    },
                    observe(take_offer),
                ))
                .with_children(|offer| {
                    let cell = || {
                        (
                            slot_node(),
                            BackgroundColor(theme::slot()),
                            BorderColor {
                                top: theme::bevel_lo(),
                                left: theme::bevel_lo(),
                                bottom: theme::bevel_hi(),
                                right: theme::bevel_hi(),
                            },
                            Pickable::IGNORE,
                        )
                    };
                    offer
                        .spawn((TradeSide::Cost, cell()))
                        .with_children(|cell| {
                            cell.spawn(slot_icon(icon(atlas)));
                            cell.spawn(slot_count(font.clone()));
                        });
                    offer.spawn(text("->"));
                    offer
                        .spawn((TradeSide::Goods, cell()))
                        .with_children(|cell| {
                            cell.spawn(slot_icon(icon(atlas)));
                            cell.spawn(slot_count(font.clone()));
                            cell.spawn((
                                DirectionsIcon,
                                ImageNode::new(assets.load(DIRECTIONS_PATH)),
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: px(ICON_SIZE),
                                    height: px(ICON_SIZE),
                                    ..default()
                                },
                                Visibility::Hidden,
                                Pickable::IGNORE,
                            ));
                        });
                    offer.spawn((TradeLeft, text("")));
                });
            }
        });
}

/// Asks the server for the offer clicked.
fn take_offer(
    event: On<Activate>,
    offers: Query<&TradeOffer, Without<InteractionDisabled>>,
    trade: Res<OpenTrade>,
    mut client: ResMut<RenetClient>,
) {
    let (Ok(offer), Some(trade)) = (offers.get(event.entity), &trade.0) else {
        return;
    };
    client.send_game_message(ClientToServerMessage::Trade {
        mob: trade.mob,
        offer: offer.0 as u8,
    });
}

/// Forgets the trade of an earlier session.
pub fn reset_trade(mut trade: ResMut<OpenTrade>) {
    trade.0 = None;
}

/// Opens, updates and closes the trade as the server says, and ends it when the panel closes.
/// A trade that opens under another dialog is ended at once.
pub fn follow_trade(
    mut updates: MessageReader<TradeUpdated>,
    mut trade: ResMut<OpenTrade>,
    ui_mode: Res<UIMode>,
    mut inventory_root: Query<&mut Visibility, With<InventoryRoot>>,
    client: Option<ResMut<RenetClient>>,
) {
    let Ok(mut panel) = inventory_root.single_mut() else {
        return;
    };
    let shown = |trade: &OpenTrade| trade.0.as_ref().map(|trade| trade.mob);
    let mut ended = shown(&trade).is_some() && *panel == Visibility::Hidden;
    for TradeUpdated(update) in updates.read() {
        let open: Option<MobId> = shown(&trade);
        match update {
            Some(view) if view.opening && *ui_mode != UIMode::Closed => ended = true,
            Some(view) if view.opening => {
                trade.0 = Some(view.clone());
                *panel = Visibility::Visible;
                ended = false;
            }
            Some(view) if open == Some(view.mob) => trade.0 = Some(view.clone()),
            Some(_) => {}
            None if open.is_some() => {
                trade.0 = None;
                *panel = Visibility::Hidden;
                ended = false;
            }
            None => {}
        }
    }
    if ended {
        trade.0 = None;
        if let Some(mut client) = client {
            client.send_game_message(ClientToServerMessage::EndTrade);
        }
    }
}

fn counted((item, nb): (ItemId, u32)) -> String {
    format!("{nb} {}", item_name(item))
}

/// Shows the offers of the open trade, disabling those that cannot be taken.
#[allow(clippy::type_complexity)]
pub fn display_trade(
    trade: Res<OpenTrade>,
    inventory: Res<Inventory>,
    materials: Res<MaterialResource>,
    mut buttons: Query<(
        Entity,
        &TradeOffer,
        &mut TabIndex,
        &mut Tooltip,
        &AccessibleLabel,
        Has<InteractionDisabled>,
        &Children,
    )>,
    sides: Query<(&TradeSide, &Children)>,
    mut lefts: Query<&mut Text, (With<TradeLeft>, Without<SlotCount>)>,
    mut counts: Query<&mut Text, With<SlotCount>>,
    mut icons: Query<(&mut ImageNode, &mut Visibility), (With<SlotIcon>, Without<DirectionsIcon>)>,
    mut directions: Query<&mut Visibility, With<DirectionsIcon>>,
    mut commands: Commands,
) {
    for (entity, &TradeOffer(index), mut tab, mut tooltip, label, disabled, children) in
        &mut buttons
    {
        let offer = trade.0.as_ref().and_then(|trade| {
            let offer = offers(trade.kind).get(index)?;
            Some((offer, trade.left.get(index).copied().unwrap_or(0)))
        });
        let Some((offer, left)) = offer else {
            tab.set_if_neq(TabIndex(-1));
            tooltip.set_if_neq(Tooltip::default());
            continue;
        };
        tab.set_if_neq(TabIndex(0));
        let refusal = if left == 0 {
            Some(TradeRefusal::SoldOut)
        } else {
            offer.applied_to(&inventory).err()
        };
        if refusal.is_some() != disabled {
            if disabled {
                commands.entity(entity).remove::<InteractionDisabled>();
            } else {
                commands.entity(entity).insert(InteractionDisabled);
            }
        }
        let title = match offer.goods {
            Goods::Items(item, nb) => counted((item, nb)),
            Goods::Directions => crate::i18n::tr(DIRECTIONS_TITLE).to_string(),
        };
        let text = trf(
            "trade.offer",
            &[("title", &title), ("cost", &counted(offer.cost))],
        );
        if label.0 != text {
            commands.entity(entity).insert(AccessibleLabel::new(text));
        }
        tooltip.set_if_neq(Tooltip {
            title,
            detail: trf("trade.for", &[("cost", &counted(offer.cost))]),
            note: String::new(),
            hint: match refusal {
                None => tr("trade.click").to_string(),
                Some(TradeRefusal::SoldOut) => tr("trade.sold_out_sunrise").to_string(),
                Some(TradeRefusal::CannotPay) => tr("trade.cannot_pay").to_string(),
                Some(TradeRefusal::NoRoom) => tr("trade.full").to_string(),
            },
        });
        for child in children.iter() {
            if let Ok(mut text) = lefts.get_mut(child) {
                let shown = match left {
                    0 => tr("trade.sold_out").to_string(),
                    left => trf("trade.left", &[("left", &left)]),
                };
                if text.0 != shown {
                    text.0 = shown;
                }
            }
            let Ok((side, cell)) = sides.get(child) else {
                continue;
            };
            let stack = match (side, offer.goods) {
                (TradeSide::Cost, _) => Some(offer.cost),
                (TradeSide::Goods, Goods::Items(item, nb)) => Some((item, nb)),
                (TradeSide::Goods, Goods::Directions) => None,
            };
            let sprite = stack.and_then(|(item, _)| {
                let atlas = materials.items.as_ref()?;
                atlas.sprite(&format!("{item:?}"))
            });
            let count = stack.map(|(_, nb)| count_label(nb)).unwrap_or_default();
            for part in cell.iter() {
                if let Ok(mut text) = counts.get_mut(part) {
                    if text.0 != count {
                        text.0.clone_from(&count);
                    }
                }
                if let Ok((mut image, mut visibility)) = icons.get_mut(part) {
                    visibility.set_if_neq(if sprite.is_some() {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    });
                    if sprite.is_some() && image.texture_atlas != sprite {
                        image.texture_atlas.clone_from(&sprite);
                    }
                }
                if let Ok(mut visibility) = directions.get_mut(part) {
                    visibility.set_if_neq(if stack.is_none() {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    });
                }
            }
        }
    }
}
