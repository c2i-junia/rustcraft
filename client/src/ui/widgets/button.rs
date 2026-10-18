//! The themed button: a `ui_widgets` button with the bevelled, raised look of the UI.
//!
//! Callers observe `On<Activate>` on the entity; disabling it is inserting `InteractionDisabled`.

use bevy::{
    ecs::entity::EntityHashSet,
    input_focus::tab_navigation::TabIndex,
    picking::hover::Hovered,
    prelude::*,
    ui::{InteractionDisabled, Pressed},
    ui_widgets::Button as UiButton,
};

use super::focus::FocusRing;
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, DANGER, DISABLED_ALPHA, FONT_M, OK, PANEL, RAISE, RAISE_HOVER,
    SPACE_L, SPACE_S, TEXT,
};

// The variants are first used by the main menu (Lot 1).
#[allow(dead_code)]
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum ButtonVariant {
    #[default]
    Default,
    /// A green call to action.
    Primary,
    /// A red, destructive action.
    Danger,
    /// No surface until the cursor is over it.
    Ghost,
}

/// Marks a button built by `button`, and keeps its look in step with its state.
#[derive(Component)]
pub struct ThemedButton(pub ButtonVariant);

/// A themed button showing `label`. Observe `On<Activate>` on it.
// First used by the main menu (Lot 1).
#[allow(dead_code)]
pub fn button(label: &str, variant: ButtonVariant, font: Handle<Font>) -> impl Bundle {
    let colors = button_colors(variant, false, false, false);
    (
        // the widget before the label: its `Role::Button` must win over the label's default node
        UiButton,
        AccessibleLabel::new(label),
        TabIndex(0),
        Hovered::default(),
        FocusRing,
        ThemedButton(variant),
        Node {
            border: UiRect::all(px(BEVEL_WIDTH)),
            padding: UiRect::axes(px(SPACE_L), px(SPACE_S)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        colors.border,
        BackgroundColor(colors.background),
        children![(
            Text::new(label),
            TextFont {
                font: font.into(),
                font_size: px(FONT_M).into(),
                ..default()
            },
            TextColor(colors.text),
        )],
    )
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (style_buttons, style_released_buttons));
}

/// The colours of a button in a given state.
struct ButtonColors {
    background: Color,
    border: BorderColor,
    text: Color,
}

fn button_colors(
    variant: ButtonVariant,
    hovered: bool,
    pressed: bool,
    disabled: bool,
) -> ButtonColors {
    // a disabled button ignores the cursor
    let (hovered, pressed) = (hovered && !disabled, pressed && !disabled);
    let surface = if hovered { RAISE_HOVER } else { RAISE };
    let (background, text) = match variant {
        ButtonVariant::Default => (surface, TEXT),
        ButtonVariant::Primary => (OK, PANEL),
        ButtonVariant::Danger => (DANGER, PANEL),
        ButtonVariant::Ghost if hovered || pressed => (RAISE_HOVER, TEXT),
        ButtonVariant::Ghost => (Color::NONE, TEXT),
    };
    let (light, dark) = if pressed {
        (BEVEL_LO, BEVEL_HI)
    } else {
        (BEVEL_HI, BEVEL_LO)
    };
    let border = if variant == ButtonVariant::Ghost && !hovered && !pressed {
        BorderColor::all(Color::NONE)
    } else {
        BorderColor {
            top: light,
            left: light,
            bottom: dark,
            right: dark,
        }
    };
    let fade = |color: Color| {
        if disabled {
            color.with_alpha(color.alpha() * DISABLED_ALPHA)
        } else {
            color
        }
    };
    ButtonColors {
        background: fade(background),
        border: BorderColor {
            top: fade(border.top),
            right: fade(border.right),
            bottom: fade(border.bottom),
            left: fade(border.left),
        },
        text: fade(text),
    }
}

type StateChanged = Or<(
    Changed<Hovered>,
    Added<Pressed>,
    Added<InteractionDisabled>,
    Added<ThemedButton>,
)>;

/// Applies the look of a button to itself and to its label.
fn apply_style(
    button: Entity,
    buttons: &mut Query<(
        &ThemedButton,
        &Hovered,
        Has<Pressed>,
        Has<InteractionDisabled>,
        &mut BackgroundColor,
        &mut BorderColor,
        &Children,
    )>,
    texts: &mut Query<&mut TextColor>,
) {
    let Ok((variant, hovered, pressed, disabled, mut background, mut border, children)) =
        buttons.get_mut(button)
    else {
        return;
    };
    let colors = button_colors(variant.0, hovered.0, pressed, disabled);
    background.0 = colors.background;
    *border = colors.border;
    for child in children {
        if let Ok(mut text) = texts.get_mut(*child) {
            text.0 = colors.text;
        }
    }
}

fn style_buttons(
    changed: Query<Entity, (With<ThemedButton>, StateChanged)>,
    mut buttons: Query<(
        &ThemedButton,
        &Hovered,
        Has<Pressed>,
        Has<InteractionDisabled>,
        &mut BackgroundColor,
        &mut BorderColor,
        &Children,
    )>,
    mut texts: Query<&mut TextColor>,
) {
    for button in &changed {
        apply_style(button, &mut buttons, &mut texts);
    }
}

/// Restyles the buttons that stopped being pressed or disabled.
fn style_released_buttons(
    mut released: RemovedComponents<Pressed>,
    mut enabled: RemovedComponents<InteractionDisabled>,
    mut buttons: Query<(
        &ThemedButton,
        &Hovered,
        Has<Pressed>,
        Has<InteractionDisabled>,
        &mut BackgroundColor,
        &mut BorderColor,
        &Children,
    )>,
    mut texts: Query<&mut TextColor>,
) {
    let entities: EntityHashSet = released.read().chain(enabled.read()).collect();
    for button in entities {
        apply_style(button, &mut buttons, &mut texts);
    }
}

#[cfg(test)]
mod tests {
    use accesskit::Role;
    use bevy::a11y::AccessibilityNode;

    use super::*;

    fn edges(border: &BorderColor) -> [Color; 4] {
        [border.top, border.left, border.bottom, border.right]
    }

    #[test]
    fn a_default_button_is_raised_and_lightens_on_hover() {
        let rest = button_colors(ButtonVariant::Default, false, false, false);
        assert_eq!(rest.background, RAISE);
        assert_eq!(
            edges(&rest.border),
            [BEVEL_HI, BEVEL_HI, BEVEL_LO, BEVEL_LO]
        );
        assert_eq!(rest.text, TEXT);
        let hover = button_colors(ButtonVariant::Default, true, false, false);
        assert_eq!(hover.background, RAISE_HOVER);
    }

    #[test]
    fn pressing_inverts_the_bevel() {
        let pressed = button_colors(ButtonVariant::Default, true, true, false);
        assert_eq!(
            edges(&pressed.border),
            [BEVEL_LO, BEVEL_LO, BEVEL_HI, BEVEL_HI]
        );
    }

    #[test]
    fn a_disabled_button_is_at_40_percent() {
        for variant in [
            ButtonVariant::Default,
            ButtonVariant::Primary,
            ButtonVariant::Danger,
            ButtonVariant::Ghost,
        ] {
            let normal = button_colors(variant, false, false, false);
            let disabled = button_colors(variant, false, false, true);
            let fades = |normal: Color, faded: Color| {
                assert_eq!(faded.alpha(), normal.alpha() * DISABLED_ALPHA);
            };
            fades(normal.background, disabled.background);
            fades(normal.text, disabled.text);
            for (n, d) in edges(&normal.border)
                .into_iter()
                .zip(edges(&disabled.border))
            {
                fades(n, d);
            }
            // the cursor changes nothing
            let touched = button_colors(variant, true, true, true);
            assert_eq!(touched.background, disabled.background);
            assert_eq!(edges(&touched.border), edges(&disabled.border));
            assert_eq!(touched.text, disabled.text);
        }
    }

    #[test]
    fn the_variants_have_their_colours() {
        let primary = button_colors(ButtonVariant::Primary, false, false, false);
        assert_eq!((primary.background, primary.text), (OK, PANEL));
        let danger = button_colors(ButtonVariant::Danger, false, false, false);
        assert_eq!((danger.background, danger.text), (DANGER, PANEL));
        let ghost = button_colors(ButtonVariant::Ghost, false, false, false);
        assert_eq!(ghost.background, Color::NONE);
        assert_eq!(edges(&ghost.border), [Color::NONE; 4]);
        assert_eq!(ghost.text, TEXT);
        let hover = button_colors(ButtonVariant::Ghost, true, false, false);
        assert_eq!(hover.background, RAISE_HOVER);
    }

    #[test]
    fn a_button_has_the_button_role_and_its_label() {
        let mut world = World::new();
        let id = world
            .spawn(button("Play", ButtonVariant::Default, Handle::default()))
            .id();
        let node = world.get::<AccessibilityNode>(id).unwrap();
        assert_eq!(node.role(), Role::Button);
        assert_eq!(node.label(), Some("Play"));
    }
}
