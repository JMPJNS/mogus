use bevy::prelude::*;
use bevy::{asset::AssetServerSettings, render::texture::ImageSettings};
use bevy_ecs_ldtk::prelude::*;
use bevy_inspector_egui::WorldInspectorPlugin;
use leafwing_input_manager::prelude::*;

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn_bundle(Camera2dBundle::default());

    commands.spawn_bundle(LdtkWorldBundle {
        ldtk_handle: asset_server.load("world.ldtk"),
        ..Default::default()
    });
}

fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            width: 1270.0,
            height: 720.0,
            title: String::from("Mogus"),
            ..Default::default()
        })
        .insert_resource(AssetServerSettings {
            watch_for_changes: true,
            ..default()
        })
        .insert_resource(ImageSettings::default_nearest())
        .insert_resource(LevelSelection::Index(0))
        .add_plugins(DefaultPlugins)
        .add_plugin(LdtkPlugin)
        .register_ldtk_entity::<PlayerBundle>("Player")
        .add_plugin(InputManagerPlugin::<Action>::default())
        .add_startup_system(startup)
        .add_startup_system(spawn_player)
        .add_system(jump)
        .add_system(print_health)
        .add_plugin(WorldInspectorPlugin::new())
        .run();
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Reflect, Default, Component)]
pub struct Player;

#[derive(Clone, Default, Bundle, LdtkEntity)]
pub struct PlayerBundle {
    #[bundle]
    #[sprite_sheet_bundle(
        "sprites/tilemaps/characters_packed.png",
        18.0,
        18.0,
        9,
        3,
        0.0,
        0.0,
        1
    )]
    sprite_sheet: SpriteSheetBundle,
    #[worldly]
    worldly: Worldly,
    #[grid_coords]
    grid_coords: GridCoords,
    #[from_entity_instance]
    health: HealthValue,
    #[from_entity_instance]
    entity_instance: EntityInstance,
}

#[derive(Default, Reflect, Component, Clone, Debug)]
pub struct HealthValue {
    value: f32,
}

impl From<EntityInstance> for HealthValue {
    fn from(entity_instance: EntityInstance) -> HealthValue {
        println!("creating entity from stuff {:?}", entity_instance);
        let mut health_value = 0.0;
        for field in entity_instance.field_instances {
            health_value = match field.identifier.as_str() {
                "health" => match field.value {
                    FieldValue::Float(x) => x.unwrap_or(health_value),
                    _ => health_value,
                },
                _ => health_value,
            }
        }

        HealthValue {
            value: health_value,
        }
    }
}

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug)]
enum Action {
    Run,
    Jump,
}

// TODO: figure out how to combine this player and the one from ldtk
fn spawn_player(mut commands: Commands) {
    commands
        .spawn()
        .insert(Player)
        .insert_bundle(InputManagerBundle::<Action> {
            // Stores "which actions are currently pressed"
            action_state: ActionState::default(),
            // Describes how to convert from player inputs into those actions
            input_map: InputMap::new([(KeyCode::Space, Action::Jump)]),
        });
}

// Query for the `ActionState` component in your game logic systems!
fn jump(query: Query<&ActionState<Action>, With<Player>>) {
    let action_state = query.single();
    // Each action has a button-like state of its own that you can check
    if action_state.just_pressed(Action::Jump) {
        println!("I'm jumping!");
    }
}

fn print_health(query: Query<&HealthValue>) {
    for e in query.iter() {
        println!("{:?}", e)
    }
}
