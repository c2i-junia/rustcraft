use crate::input::data::GameAction;
use crate::input::keyboard::{is_action_just_pressed, is_action_just_released};
use crate::network::CachedChatConversation;
use crate::network::SendGameMessageExtension;
use crate::state::KeyMap;
use crate::ui::assets::load_font;
use crate::ui::hud::UiDialog;
use crate::ui::style::text_font;
use bevy::prelude::*;
use bevy_renet::RenetClient;
use bevy_simple_text_input::*;
use shared::messages::{ChatMessageRequest, ClientToServerMessage};
use shared::utils::unix_time_ms;

use super::UIMode;

#[derive(Component)]
pub struct ChatRoot;

#[derive(Component)]
pub struct ChatDisplay;

#[derive(Component)]
pub struct ChatInput;

#[derive(Component)]
pub struct MessageAnimator {
    created_ts: u64,
}

const CHAT_COLOR: Color = Color::srgba(0., 0., 0., 0.6);
const CHAT_SIZE: f32 = 17.;
const CHAT_MAX_MESSAGES: usize = 2;

// Time in ms
const ANIMATION_BEGIN_FADE: u64 = 5_000;
const ANIMATION_HIDE: u64 = 2_000;

pub fn setup_chat(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            Name::new("ChatRoot"),
            DespawnOnExit(crate::state::GameState::Game),
            ChatRoot,
            UiDialog,
            (
                Node {
                    display: Display::Flex,
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.),
                    max_height: Val::Px((CHAT_MAX_MESSAGES as f32 + 20.) * CHAT_SIZE),
                    width: Val::Vw(20.),
                    left: Val::Percent(0.),
                    column_gap: Val::Px(0.),
                    overflow: Overflow {
                        x: OverflowAxis::Visible,
                        y: OverflowAxis::Hidden,
                    },
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(CHAT_COLOR),
                Visibility::Hidden,
            ),
        ))
        .with_children(|root| {
            root.spawn((
                ChatDisplay,
                (Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::End,
                    column_gap: Val::Px(0.),
                    overflow: Overflow {
                        x: OverflowAxis::Visible,
                        y: OverflowAxis::Hidden,
                    },
                    width: Val::Percent(100.),
                    ..default()
                },),
            ));

            root.spawn((
                ChatInput,
                (Node {
                    width: Val::Percent(100.),
                    ..default()
                },),
                (
                    TextInput,
                    TextInputValue("".into()),
                    TextInputPlaceholder {
                        value: "Send a message...".to_string(),
                        ..default()
                    },
                    TextInputTextFont(text_font(load_font(&asset_server), CHAT_SIZE)),
                    TextInputTextColor(TextColor(Color::WHITE)),
                    TextInputInactive(true),
                ),
            ));
        });
}

/// Opens the chat on its key when no dialog is open, and closes it on Escape.
pub fn toggle_chat_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    ui_mode: Res<UIMode>,
    mut chat_root: Query<&mut Visibility, With<ChatRoot>>,
    mut chat_input: Query<(&mut TextInputInactive, &mut TextInputValue), With<ChatInput>>,
) {
    let (Ok(mut visibility), Ok((mut inactive, mut value))) =
        (chat_root.single_mut(), chat_input.single_mut())
    else {
        return;
    };

    // On release, so that the key that opens the chat is not typed into it
    if *ui_mode == UIMode::Closed
        && is_action_just_released(GameAction::OpenChat, &keyboard_input, &key_map)
    {
        inactive.0 = false;
        *visibility = Visibility::Visible;
    }

    if *visibility == Visibility::Visible
        && is_action_just_pressed(GameAction::Escape, &keyboard_input, &key_map)
    {
        *visibility = Visibility::Hidden;
        *value = TextInputValue(String::new());
        *inactive = TextInputInactive(true);
    }
}

/// Fades out the messages shown over the game while the chat is closed. Once the chat is
/// open, or the fade is over, a message is an ordinary line of the chat again.
pub fn fade_chat_messages_system(
    mut commands: Commands,
    chat_root: Query<&Visibility, With<ChatRoot>>,
    mut messages: Query<
        (
            Entity,
            &mut BackgroundColor,
            &mut Visibility,
            &MessageAnimator,
        ),
        Without<ChatRoot>,
    >,
) {
    let chat_is_open = chat_root
        .single()
        .is_ok_and(|visibility| *visibility == Visibility::Visible);
    let now = unix_time_ms();

    for (entity, mut background, mut visibility, animator) in messages.iter_mut() {
        let age = now.saturating_sub(animator.created_ts);
        if chat_is_open || age > ANIMATION_BEGIN_FADE + ANIMATION_HIDE {
            commands.entity(entity).remove::<MessageAnimator>();
            *visibility = Visibility::Inherited;
            *background = BackgroundColor(Color::BLACK.with_alpha(0.));
        } else if age > ANIMATION_BEGIN_FADE {
            let faded = (age - ANIMATION_BEGIN_FADE) as f32 / ANIMATION_HIDE as f32;
            *background = BackgroundColor(CHAT_COLOR.with_alpha(CHAT_COLOR.alpha() * (1. - faded)));
        }
    }
}

/// Adds the messages received since the last frame to the chat, and drops the oldest ones
/// beyond `CHAT_MAX_MESSAGES`.
pub fn display_chat_messages_system(
    mut commands: Commands,
    mut conversation: ResMut<CachedChatConversation>,
    asset_server: Res<AssetServer>,
    chat_display: Query<(Entity, Option<&Children>), With<ChatDisplay>>,
) {
    let Ok((display, children)) = chat_display.single() else {
        return;
    };

    let now = unix_time_ms();
    for message in conversation.unread.drain(..) {
        commands.entity(display).with_child((
            MessageAnimator { created_ts: now },
            Text::new(format!("<{}> : {}", message.author, message.content)),
            text_font(load_font(&asset_server), CHAT_SIZE),
            TextColor(Color::WHITE),
            Visibility::Visible,
            BackgroundColor(CHAT_COLOR),
        ));
    }

    // The oldest messages come first. Those added just above are counted from the next frame.
    let children = children.map(|children| &children[..]).unwrap_or_default();
    let too_many = children.len().saturating_sub(CHAT_MAX_MESSAGES);
    for &oldest in &children[..too_many] {
        commands.entity(oldest).despawn();
    }
}

/// Sends the message typed in the chat when it is submitted, and closes the chat.
pub fn send_chat_message_system(
    mut submitted: MessageReader<TextInputSubmitMessage>,
    mut client: ResMut<RenetClient>,
    mut chat_root: Query<&mut Visibility, With<ChatRoot>>,
    mut chat_input: Query<(Entity, &mut TextInputInactive), With<ChatInput>>,
) {
    let (Ok(mut visibility), Ok((input, mut inactive))) =
        (chat_root.single_mut(), chat_input.single_mut())
    else {
        return;
    };

    for message in submitted.read() {
        if message.entity != input {
            continue;
        }
        *visibility = Visibility::Hidden;
        *inactive = TextInputInactive(true);
        if !message.value.trim().is_empty() {
            client.send_game_message(ClientToServerMessage::ChatMessage(ChatMessageRequest {
                content: message.value.clone(),
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::messages::FullChatMessage;

    fn message(content: &str) -> FullChatMessage {
        FullChatMessage {
            content: content.into(),
            author: "steve".into(),
            timestamp: 0,
        }
    }

    #[test]
    fn the_chat_shows_new_messages_and_keeps_only_the_latest() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .init_resource::<CachedChatConversation>()
            .add_systems(Update, display_chat_messages_system);
        let display = app.world_mut().spawn(ChatDisplay).id();

        let extra = 2;
        for index in 0..CHAT_MAX_MESSAGES + extra {
            app.world_mut()
                .resource_mut::<CachedChatConversation>()
                .unread
                .push(message(&index.to_string()));
            app.update();
        }
        // The excess is dropped on the frame after it was added
        app.update();

        let shown: Vec<String> = app
            .world()
            .get::<Children>(display)
            .unwrap()
            .iter()
            .map(|line| app.world().get::<Text>(line).unwrap().0.clone())
            .collect();
        let expected: Vec<String> = (extra..CHAT_MAX_MESSAGES + extra)
            .map(|index| format!("<steve> : {index}"))
            .collect();
        assert_eq!(shown, expected);
        assert!(app
            .world()
            .resource::<CachedChatConversation>()
            .unread
            .is_empty());
    }
}
