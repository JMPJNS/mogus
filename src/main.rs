use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::utils::{HashMap, HashSet};
use bevy::{asset::AssetServerSettings, render::texture::ImageSettings};
use bevy_ecs_ldtk::prelude::*;
use bevy_inspector_egui::WorldInspectorPlugin;
use bevy_rapier2d::prelude::*;
use leafwing_input_manager::prelude::*;

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn_bundle(Camera2dBundle {
        ..Default::default()
    });
    // commands.spawn_bundle(OrthographicCameraBundle::new_2d());

    commands.spawn_bundle(LdtkWorldBundle {
        ldtk_handle: asset_server.load("world.ldtk"),
        ..Default::default()
    });
}

fn main() {
    App::new()
        .insert_resource(ImageSettings::default_nearest())
        .insert_resource(WindowDescriptor {
            width: 1920.0,
            height: 1080.0,
            title: String::from("Mogus"),
            ..Default::default()
        })
        .insert_resource(AssetServerSettings {
            watch_for_changes: true,
            ..default()
        })
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        // .add_plugin(RapierDebugRenderPlugin::default())
        .insert_resource(LdtkSettings {
            level_spawn_behavior: LevelSpawnBehavior::UseWorldTranslation {
                load_level_neighbors: true,
            },
            set_clear_color: SetClearColor::No,
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(LdtkPlugin)
        .insert_resource(LevelSelection::Uid(0))
        .register_ldtk_entity::<PlayerBundle>("Player")
        .register_ldtk_int_cell::<WallBundle>(1)
        .register_ldtk_int_cell::<WallBundle>(4)
        .add_plugin(InputManagerPlugin::<Action>::default())
        .add_startup_system(startup)
        .add_system(spawn_wall_collision)
        .add_system(init_player)
        .add_system(jump)
        .add_system(move_player)
        .add_system(set_current_level)
        .add_system(camera_fit_inside_current_level)
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_plugin(WorldInspectorPlugin::new())
        .run();
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Reflect, Default, Component)]
pub struct Player {
    pub current_level: usize,
}

#[derive(Clone, Default, Bundle, LdtkEntity)]
pub struct PlayerBundle {
    #[bundle]
    #[sprite_sheet_bundle(
        "sprites/tilemaps/characters_packed.png",
        24.0,
        24.0,
        9,
        3,
        0.0,
        0.0,
        1
    )]
    sprite_sheet: SpriteSheetBundle,
    #[worldly]
    worldly: Worldly,
    #[from_entity_instance]
    health: HealthValue,
    #[from_entity_instance]
    entity_instance: EntityInstance,
    player: Player,
    velocity: Velocity,
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

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default, Component)]
pub struct Wall;

#[derive(Clone, Debug, Default, Bundle, LdtkIntCell)]
pub struct WallBundle {
    wall: Wall,
}

pub fn spawn_wall_collision(
    mut commands: Commands,
    wall_query: Query<(&GridCoords, &Parent), Added<Wall>>,
    parent_query: Query<&Parent, Without<Wall>>,
    level_query: Query<(Entity, &Handle<LdtkLevel>)>,
    levels: Res<Assets<LdtkLevel>>,
) {
    /// Represents a wide wall that is 1 tile tall
    /// Used to spawn wall collisions
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Default, Hash)]
    struct Plate {
        left: i32,
        right: i32,
    }

    /// A simple rectangle type representing a wall of any size
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Default, Hash)]
    struct Rect {
        left: i32,
        right: i32,
        top: i32,
        bottom: i32,
    }

    // Consider where the walls are
    // storing them as GridCoords in a HashSet for quick, easy lookup
    //
    // The key of this map will be the entity of the level the wall belongs to.
    // This has two consequences in the resulting collision entities:
    // 1. it forces the walls to be split along level boundaries
    // 2. it lets us easily add the collision entities as children of the appropriate level entity
    let mut level_to_wall_locations: HashMap<Entity, HashSet<GridCoords>> = HashMap::new();

    wall_query.for_each(|(&grid_coords, parent)| {
        // An intgrid tile's direct parent will be a layer entity, not the level entity
        // To get the level entity, you need the tile's grandparent.
        // This is where parent_query comes in.
        if let Ok(grandparent) = parent_query.get(parent.get()) {
            level_to_wall_locations
                .entry(grandparent.get())
                .or_insert(HashSet::new())
                .insert(grid_coords);
        }
    });

    if !wall_query.is_empty() {
        level_query.for_each(|(level_entity, level_handle)| {
            if let Some(level_walls) = level_to_wall_locations.get(&level_entity) {
                let level = levels
                    .get(level_handle)
                    .expect("Level should be loaded by this point");

                let LayerInstance {
                    c_wid: width,
                    c_hei: height,
                    grid_size,
                    ..
                } = level
                    .level
                    .layer_instances
                    .clone()
                    .expect("Level asset should have layers")[0];

                // combine wall tiles into flat "plates" in each individual row
                let mut plate_stack: Vec<Vec<Plate>> = Vec::new();

                for y in 0..height {
                    let mut row_plates: Vec<Plate> = Vec::new();
                    let mut plate_start = None;

                    // + 1 to the width so the algorithm "terminates" plates that touch the right
                    // edge
                    for x in 0..width + 1 {
                        match (plate_start, level_walls.contains(&GridCoords { x, y })) {
                            (Some(s), false) => {
                                row_plates.push(Plate {
                                    left: s,
                                    right: x - 1,
                                });
                                plate_start = None;
                            }
                            (None, true) => plate_start = Some(x),
                            _ => (),
                        }
                    }

                    plate_stack.push(row_plates);
                }

                // combine "plates" into rectangles across multiple rows
                let mut wall_rects: Vec<Rect> = Vec::new();
                let mut previous_rects: HashMap<Plate, Rect> = HashMap::new();

                // an extra empty row so the algorithm "terminates" the rects that touch the top
                // edge
                plate_stack.push(Vec::new());

                for (y, row) in plate_stack.iter().enumerate() {
                    let mut current_rects: HashMap<Plate, Rect> = HashMap::new();
                    for plate in row {
                        if let Some(previous_rect) = previous_rects.remove(plate) {
                            current_rects.insert(
                                *plate,
                                Rect {
                                    top: previous_rect.top + 1,
                                    ..previous_rect
                                },
                            );
                        } else {
                            current_rects.insert(
                                *plate,
                                Rect {
                                    bottom: y as i32,
                                    top: y as i32,
                                    left: plate.left,
                                    right: plate.right,
                                },
                            );
                        }
                    }

                    // Any plates that weren't removed above have terminated
                    wall_rects.append(&mut previous_rects.values().copied().collect());
                    previous_rects = current_rects;
                }

                commands.entity(level_entity).with_children(|level| {
                    // Spawn colliders for every rectangle..
                    // Making the collider a child of the level serves two purposes:
                    // 1. Adjusts the transforms to be relative to the level for free
                    // 2. the colliders will be despawned automatically when levels unload
                    for wall_rect in wall_rects {
                        level
                            .spawn()
                            .insert(Collider::cuboid(
                                (wall_rect.right as f32 - wall_rect.left as f32 + 1.)
                                    * grid_size as f32
                                    / 2.,
                                (wall_rect.top as f32 - wall_rect.bottom as f32 + 1.)
                                    * grid_size as f32
                                    / 2.,
                            ))
                            .insert(RigidBody::Fixed)
                            .insert(Friction {
                                coefficient: 0.1,
                                ..Default::default()
                            })
                            .insert(Transform::from_xyz(
                                (wall_rect.left + wall_rect.right + 1) as f32 * grid_size as f32
                                    / 2.,
                                (wall_rect.bottom + wall_rect.top + 1) as f32 * grid_size as f32
                                    / 2.,
                                0.,
                            ))
                            .insert(GlobalTransform::default());
                    }
                });
            }
        });
    }
}

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug)]
enum Action {
    Run,
    Jump,
    Left,
    Right,
}

fn init_player(mut commands: Commands, query: Query<Entity, Added<Player>>) {
    if let Ok(entity) = query.get_single() {
        commands
            .entity(entity)
            .insert(Player { current_level: 0 })
            .insert(RigidBody::Dynamic)
            .insert(Collider::cuboid(12.0, 12.0))
            .insert_bundle(InputManagerBundle::<Action> {
                // Stores "which actions are currently pressed"
                action_state: ActionState::default(),
                // Describes how to convert from player inputs into those actions
                input_map: InputMap::new([
                    // Jump
                    (KeyCode::Space, Action::Jump),
                    (KeyCode::Up, Action::Jump),
                    (KeyCode::W, Action::Jump),
                    // Left
                    (KeyCode::Left, Action::Left),
                    (KeyCode::A, Action::Left),
                    // Right
                    (KeyCode::Right, Action::Right),
                    (KeyCode::D, Action::Right),
                ]),
            });
    }
}

fn jump(mut query: Query<(&ActionState<Action>, &mut Velocity), With<Player>>) {
    if let Ok((action_state, mut velocity)) = query.get_single_mut() {
        if action_state.pressed(Action::Jump) {
            velocity.linvel = Vec2::new(velocity.linvel.x, 64.0);
        }
    }
}

fn move_player(mut query: Query<(&ActionState<Action>, &mut Velocity), With<Player>>) {
    if let Ok((action_state, mut velocity)) = query.get_single_mut() {
        if action_state.pressed(Action::Right) {
            velocity.linvel = Vec2::new(50.0, velocity.linvel.y);
        }

        if action_state.pressed(Action::Left) {
            velocity.linvel = Vec2::new(-50.0, velocity.linvel.y);
        }
    }
}

const ASPECT_RATIO: f32 = 16. / 9.;

pub fn set_current_level(
    mut commands: Commands,
    mut player_query: Query<(&Transform, &mut Player)>,
    level_query: Query<&Handle<LdtkLevel>, (Without<OrthographicProjection>, Without<Player>)>,
    ldtk_levels: Res<Assets<LdtkLevel>>,
    level_selection: Res<LevelSelection>,
) {
    if let Ok((player_transform, mut player)) = player_query.get_single_mut() {
        for level_handle in level_query.iter() {
            if let Some(ldtk_level) = ldtk_levels.get(level_handle) {
                let level = &ldtk_level.level;

                // check in which level player is
                if (player_transform.translation.x >= level.world_x as f32
                    && player_transform.translation.x <= (level.world_x + level.px_wid) as f32)
                    && (player_transform.translation.y >= level.world_y as f32
                        && player_transform.translation.y <= (level.world_y + level.px_hei) as f32)
                {
                    player.current_level = level.uid.try_into().unwrap_or(0);
                }
                let new_level = LevelSelection::Uid(player.current_level as i32);
                if level_selection.ne(&new_level) {
                    println!("switching to level {:?}", level.identifier);
                    commands.insert_resource(new_level);
                }
            }
        }
    }
}

pub fn camera_fit_inside_current_level(
    mut camera_query: Query<
        (
            &mut bevy::render::camera::OrthographicProjection,
            &mut Transform,
        ),
        Without<Player>,
    >,
    mut player_query: Query<(&Transform, &mut Player)>,
    level_query: Query<
        (&Transform, &Handle<LdtkLevel>),
        (Without<OrthographicProjection>, Without<Player>),
    >,
    level_selection: Res<LevelSelection>,
    ldtk_levels: Res<Assets<LdtkLevel>>,
) {
    if let Ok((player_transform, mut player)) = player_query.get_single_mut() {
        let (mut orthographic_projection, mut camera_transform) = camera_query.single_mut();
        for (level_transform, level_handle) in level_query.iter() {
            if let Some(ldtk_level) = ldtk_levels.get(level_handle) {
                let level = &ldtk_level.level;

                if level_selection.is_match(&player.current_level, level) {
                    let level_ratio = level.px_wid as f32 / level.px_hei as f32;

                    orthographic_projection.scaling_mode = bevy::render::camera::ScalingMode::None;
                    orthographic_projection.bottom = 0.;
                    orthographic_projection.left = 0.;
                    if level_ratio > ASPECT_RATIO {
                        // level is wider than the screen
                        orthographic_projection.top = (level.px_hei as f32 / 9.).round() * 9.;
                        orthographic_projection.right = orthographic_projection.top * ASPECT_RATIO;
                        camera_transform.translation.x = (player_transform.translation.x
                            - level_transform.translation.x
                            - orthographic_projection.right / 2.)
                            .clamp(0., level.px_wid as f32 - orthographic_projection.right);
                        camera_transform.translation.y = 0.;
                    } else {
                        // level is taller than the screen
                        orthographic_projection.right = (level.px_wid as f32 / 16.).round() * 16.;
                        orthographic_projection.top = orthographic_projection.right / ASPECT_RATIO;
                        camera_transform.translation.y = (player_transform.translation.y
                            - level_transform.translation.y
                            - orthographic_projection.top / 2.)
                            .clamp(0., level.px_hei as f32 - orthographic_projection.top);
                        camera_transform.translation.x = 0.;
                    }

                    camera_transform.translation.x += level_transform.translation.x;
                    camera_transform.translation.y += level_transform.translation.y;
                }
            }
        }
    }
}
