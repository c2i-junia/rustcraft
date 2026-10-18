//! Whether a dialog or a text field currently takes the player's inputs.

use bevy::{
    input_focus::InputFocus,
    log::debug,
    prelude::{Component, Query, Res, ResMut, Resource, Visibility, With},
    text::EditableText,
};

/// All UI dialogs toggling mouse visibility MUST use this in their bundle list\
/// They must also possess the `visibility` attribute\
/// Basically used to detect if multiple dialogs are open at once\
/// For example, mouse visibility : must stay visible as long as at least one dialog is active\
/// When the last active dialog is hidden, the mouse too\
#[derive(Component)]
pub struct UiDialog;

#[derive(PartialEq, Eq, Clone, Copy, Resource)]
pub enum UIMode {
    Opened,
    Closed,
    /// When the focus is on a text field: the user is typing, and all other inputs shall be
    /// ignored
    Typing,
}

pub fn set_ui_mode(
    mut ui_mode: ResMut<UIMode>,
    visibility: Query<&Visibility, With<UiDialog>>,
    focus: Res<InputFocus>,
    editable: Query<(), With<EditableText>>,
) {
    if focus.get().is_some_and(|entity| editable.contains(entity)) {
        debug!("Typing...");
        *ui_mode = UIMode::Typing;
        return;
    }
    for vis in visibility.iter() {
        if vis == Visibility::Visible {
            *ui_mode = UIMode::Opened;
            return;
        }
    }
    *ui_mode = UIMode::Closed;
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;

    fn mode(app: &App) -> UIMode {
        *app.world().resource::<UIMode>()
    }

    #[test]
    fn typing_is_the_focus_on_an_editable_text() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<InputFocus>()
            .insert_resource(UIMode::Closed)
            .add_systems(Update, set_ui_mode);
        let field = app.world_mut().spawn(EditableText::default()).id();
        let other = app.world_mut().spawn_empty().id();

        app.insert_resource(InputFocus::from_entity(field));
        app.update();
        assert!(mode(&app) == UIMode::Typing);

        app.insert_resource(InputFocus::from_entity(other));
        app.update();
        assert!(mode(&app) == UIMode::Closed);

        app.world_mut().resource_mut::<InputFocus>().clear();
        app.world_mut().spawn((UiDialog, Visibility::Visible));
        app.update();
        assert!(mode(&app) == UIMode::Opened);
    }
}
