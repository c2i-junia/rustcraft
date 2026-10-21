use crate::world::weather::ClientWeather;
use bevy::prelude::*;

#[derive(Component)]
pub struct WeatherText;

/// `Weather: rain 0.85`: the weather the server told, then the rain level, and the thunder
/// level after a slash while there is some.
pub fn weather_text_update_system(
    weather: Res<ClientWeather>,
    query: Query<Entity, With<WeatherText>>,
    mut writer: TextUiWriter,
) {
    let mut line = format!("Weather: {} {:.2}", weather.kind.name(), weather.rain);
    if weather.thunder > 0.0 {
        line += &format!("/{:.2}", weather.thunder);
    }
    for entity in query.iter() {
        *writer.text(entity, 0) = line.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use shared::world::WeatherKind;

    #[test]
    fn the_overlay_shows_the_weather_and_its_levels() {
        let mut world = World::new();
        world.init_resource::<bevy::text::TextIterScratch>();
        let text = world.spawn((WeatherText, Text::new("Weather: N/A"))).id();
        let mut weather = ClientWeather::settled(WeatherKind::Clear);
        weather.kind = WeatherKind::Rain;
        weather.rain = 0.854;
        world.insert_resource(weather.clone());
        world.run_system_once(weather_text_update_system).unwrap();
        assert_eq!(world.get::<Text>(text).unwrap().0, "Weather: rain 0.85");

        weather.kind = WeatherKind::Thunder;
        weather.thunder = 0.4;
        world.insert_resource(weather);
        world.run_system_once(weather_text_update_system).unwrap();
        assert_eq!(
            world.get::<Text>(text).unwrap().0,
            "Weather: thunder 0.85/0.40"
        );
    }
}
