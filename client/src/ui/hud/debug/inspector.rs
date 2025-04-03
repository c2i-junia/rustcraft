use bevy::{prelude::*, window::PrimaryWindow};
use bevy_inspector_egui::bevy_egui::EguiContext;

pub fn inspector_ui(world: &mut World) {
    let Ok(egui_context) = world
        .query_filtered::<&mut EguiContext, With<PrimaryWindow>>()
        .get_single(world)
    else {
        return;
    };
    let mut egui_context = egui_context.clone();

    let window = world.query::<&Window>().get_single(world).unwrap();
    let width = window.width();

    let time = world.get_resource::<Time>().unwrap();

    let mut res = egui::Window::new("Debug UI").default_open(false);

    // This is kinda ridiculous, but it's the only way I found to make the window properly appears at the left side of the screen
    // Using .default_pos(w, h) doesn't fully work for some reason on a dual-monitor setup
    // A better way might be to resize this proportionally to the window width on window resize
    if time.elapsed_secs() < 1.0 {
        res = res.current_pos((width - 200.0, 0.0));
    }

    res.show(egui_context.get_mut(), |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            bevy_inspector_egui::bevy_inspector::ui_for_world(world, ui);

            egui::CollapsingHeader::new("Materials").show(ui, |ui| {
                bevy_inspector_egui::bevy_inspector::ui_for_assets::<StandardMaterial>(world, ui);
            });
        });
    });
}
