//! The background of the menus: the dirt texture, darkened, tiled in big crisp texels, under a
//! vignette.

use bevy::prelude::*;

use crate::ui::assets::BACKGROUND_IMAGE_PATH;
use crate::ui::theme::{BEVEL_LO, DIRT_TINT};

/// The size of a texel of the dirt texture, in logical pixels. Bevy measures the tiles in the
/// units of the scaled UI, so a texel covers `BACKGROUND_TEXEL_PX × UiScale` physical pixels: a
/// whole number at every scale, with no system to keep it so.
pub const BACKGROUND_TEXEL_PX: f32 = 4.0;

/// A background over the whole window: the darkened dirt, and a vignette over it. Spawn the
/// screen's content as its children.
pub fn menu_background(assets: &AssetServer) -> impl Bundle {
    (
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        ImageNode {
            image: assets.load(BACKGROUND_IMAGE_PATH),
            color: DIRT_TINT,
            image_mode: NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: BACKGROUND_TEXEL_PX,
            },
            ..default()
        },
        children![(
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundGradient::from(RadialGradient {
                stops: vec![
                    ColorStop::percent(Color::NONE, 45),
                    ColorStop::percent(BEVEL_LO.with_alpha(0.85), 100),
                ],
                ..default()
            }),
            Pickable::IGNORE,
        )],
    )
}
