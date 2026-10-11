use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContext, PrimaryEguiContext};

pub fn inspector_ui(world: &mut World, mut last_context: Local<Option<Entity>>) {
    let Ok((context_entity, egui_context)) = world
        .query_filtered::<(Entity, &mut EguiContext), With<PrimaryEguiContext>>()
        .single(world)
    else {
        return;
    };
    let mut egui_context = egui_context.clone();
    // The menu and game cameras each carry their own egui context, and a new context starts
    // with empty memory: the window has to be placed again
    let new_context = *last_context != Some(context_entity);
    *last_context = Some(context_entity);

    // The window can be missing while the app is closing
    let Ok(window) = world.query::<&Window>().single(world) else {
        return;
    };
    let width = window.width();

    let time = world.get_resource::<Time>().unwrap();

    let mut res = egui::Window::new("Debug UI").default_open(false);

    // This is kinda ridiculous, but it's the only way I found to make the window properly appears at the left side of the screen
    // Using .default_pos(w, h) doesn't fully work for some reason on a dual-monitor setup
    // A better way might be to resize this proportionally to the window width on window resize
    if new_context || time.elapsed_secs() < 1.0 {
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
