//! Load a cubemap texture onto a cube like a skybox and cycle through different compressed texture formats

#[path = "./camera_controller.rs"]
mod camera_controller;

use bevy::{
    asset::LoadState, core_pipeline::Skybox, input::common_conditions::input_just_pressed, math::Vec3A, prelude::*, render::{
        primitives::Aabb, render_resource::{TextureViewDescriptor, TextureViewDimension}
    }, window::{close_on_esc, WindowMode}
};
use camera_controller::{CameraController, CameraControllerPlugin};
use std::f32::consts::PI;
use rand::prelude::*;


#[derive(Component)]
struct Velocity {
    velocity: Vec3,
    max_velocity: f32,
    turn_speed: f32,
}

#[derive(Component)]
struct ModelEntity;

fn move_by_velocity(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &Velocity)>,
    mut gizmos: Gizmos
) {
    gizmos.arrow(Vec3::ZERO, Vec3::X * 10.0, Color::RED);
    gizmos.arrow(Vec3::ZERO, Vec3::Y * 10.0, Color::GREEN);
    gizmos.arrow(Vec3::ZERO, Vec3::Z * 10.0, Color::BLUE);

    for (mut transform, velocity) in &mut query {
        transform.translation += velocity.velocity.normalize() * time.delta_seconds();

        let target = transform.looking_to(-velocity.velocity, Vec3::Y);
        transform.rotation = transform.rotation.lerp(target.rotation, velocity.turn_speed * time.delta_seconds());

        gizmos.arrow(transform.translation, transform.translation + velocity.velocity, Color::WHITE);
        //gizmos.arrow(transform.translation, transform.translation + Vec3::Z*3.0, Color::YELLOW);
    }
}


fn update_velocity(
    time: Res<Time>,
    mut query: Query<(&Transform, &mut Velocity)>,
) {
    for (transform, mut velocity) in &mut query {
    }
}



fn adjust_by_aabb(
    mut query: Query<(Entity, &mut Transform), With<ModelEntity>>,
    children: Query<&Children>,
    bounding_boxes: Query<&Aabb>,
) {
    for (entity, mut transform) in &mut query {
        let mut min = Vec3A::MAX;
        let mut max = Vec3A::MIN;
        let mut count = 0;
        for child in children.iter_descendants(entity) {
            let Ok(bb) = bounding_boxes.get(child) else { continue };
            min = min.min(bb.center - bb.half_extents);
            max = max.max(bb.center + bb.half_extents);
            count += 1;
        }
        if count > 0 {
            let center = (min + max) * 0.5;
            transform.translation = transform.with_translation(Vec3::ZERO).transform_point(Vec3::from(-center));
        }
    }
}



fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(
            WindowPlugin {
                primary_window: Some(Window {
                    resizable: false,
                    mode: WindowMode::BorderlessFullscreen,
                    ..default()
                }),
                ..default()
            }
        ))
        .add_plugins(CameraControllerPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                toggle_pause.run_if(input_just_pressed(KeyCode::Space)),
                update_velocity,
                move_by_velocity.after(update_velocity),
                adjust_by_aabb,
                skybox_system,
                close_on_esc
            ),
        )
        .run();
}



fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // directional 'sun' light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 32000.0,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 20.0, 0.0)
            .with_rotation(Quat::from_rotation_x(-PI / 4.)),
        ..default()
    });
    
    // skybox resource
    let image_handle = asset_server.load("space_cubemap.png");
    commands.insert_resource(SkyboxResource {
        is_loaded: false,
        image_handle: image_handle.clone(),
    });

    // camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 10.0, 0.0).looking_at(Vec3::new(10.0, 0.0, 10.0), Vec3::Y),
            ..default()
        },
        CameraController {
            ..default()
        },
        Skybox {
            image: image_handle,
            brightness: 1000.0,
        },
    ));

    // ambient light
    // NOTE: The ambient light is used to scale how bright the environment map is so with a bright
    // environment map, use an appropriate color and brightness to match
    commands.insert_resource(AmbientLight {
        color: Color::rgb_u8(210, 220, 240),
        brightness: 1.0,
    });

    let destroyer_scene = asset_server.load("destroyer.glb#Scene0");
    let lowpoly2_scene = asset_server.load("lowpoly2.glb#Scene0");
    let mut rng = rand::thread_rng();
    for _ in 0..100 {
        let position = Vec3::new((rng.gen::<f32>() - 0.5) * 100.0, 0.0, (rng.gen::<f32>() - 0.5) * 100.0);
        let velocity_mag = rng.gen::<f32>() * 10.0;
        let velocity = Vec3::new(rng.gen::<f32>() - 0.5, 0.0, rng.gen::<f32>() - 0.5).normalize() * velocity_mag;

        if rng.gen_bool(0.1) {
            spawn_ship(&mut commands, destroyer_scene.clone(), position, velocity, 0.0, 0.0001);
        } else {
            spawn_ship(&mut commands, lowpoly2_scene.clone(), position, velocity, PI*0.5, 0.1);
        }
    }
}


fn spawn_ship(commands: &mut Commands, scene: Handle<Scene>, position: Vec3, velocity: Vec3, angle: f32, scale: f32) {
    commands.spawn((
        SpatialBundle {
            transform: Transform::from_translation(position),
            ..default()
        },
        Velocity {
            velocity,
            max_velocity: 10.0,
            turn_speed: 1.0
        }
    )).with_children(|parent| {
        parent.spawn((
            ModelEntity,
            SceneBundle {
                scene,
                transform: Transform::from_rotation(Quat::from_axis_angle(Vec3::Y, angle)).with_scale(Vec3::ONE * scale),
                ..default()
            }
        ));
    });
}


#[derive(Resource)]
struct SkyboxResource {
    is_loaded: bool,
    image_handle: Handle<Image>,
}

fn skybox_system(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut skybox: ResMut<SkyboxResource>,
) {
    if !skybox.is_loaded && asset_server.load_state(&skybox.image_handle) == LoadState::Loaded {
        skybox.is_loaded = true;
        let image = images.get_mut(&skybox.image_handle).unwrap();
        // NOTE: PNGs do not have any metadata that could indicate they contain a cubemap texture,
        // so they appear as one texture. The following code reconfigures the texture as necessary.
        if image.texture_descriptor.array_layer_count() == 1 {
            image.reinterpret_stacked_2d_as_array(image.height() / image.width());
            image.texture_view_descriptor = Some(TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                ..default()
            });
        }
    }
}


fn toggle_pause(mut time: ResMut<Time<Virtual>>) {
    if time.is_paused() {
        time.unpause();
    } else {
        time.pause();
    }
}



/*use bevy::{log::LogPlugin, prelude::*, window::PrimaryWindow};
use bevy_spatial::{
    kdtree::KDTree3, AutomaticUpdate, SpatialAccess, SpatialStructure, TransformMode,
};
use std::time::Duration;


#[derive(Component, Default)]
struct NearestNeighbour;

#[derive(Component)]
struct MoveTowards;



#[derive(Resource)]
struct RvoContext {
}

#[derive(Component)]
struct Velocity {
    velocity: Vec3,
    max_velocity: f32,
    agent: usize,
}


fn main() {
    App::new()
        .add_plugins(DefaultPlugins.build().disable::<LogPlugin>())
        .add_plugins(
            AutomaticUpdate::<NearestNeighbour>::new()
                .with_spatial_ds(SpatialStructure::KDTree3)
                .with_frequency(Duration::from_secs(1))
                .with_transform(TransformMode::Transform),
        )
        .add_systems(Startup, setup)
        .add_systems(Update, mouseclick)
        .add_systems(Update, move_to)
        .run();
}

type NNTree = KDTree3<NearestNeighbour>;

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    for x in -6..6 {
        for y in -6..6 {
            commands.spawn((
                NearestNeighbour,
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::rgb(0.7, 0.3, 0.5),
                        custom_size: Some(Vec2::new(10.0, 10.0)),
                        ..default()
                    },
                    transform: Transform {
                        translation: Vec3::new((x * 100) as f32, (y * 100) as f32, 0.0),
                        ..default()
                    },
                    ..default()
                },
            ));
        }
    }
}

fn mouseclick(
    mut commands: Commands,
    mouse_input: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    cam: Query<(&Camera, &GlobalTransform)>,
) {
    let win = window.single();
    let (cam, cam_t) = cam.single();
    if mouse_input.just_pressed(MouseButton::Left) {
        if let Some(pos) = win.cursor_position() {
            commands.spawn((
                MoveTowards,
                Velocity {
                    velocity: Vec3::ZERO,
                    max_velocity: 10.0,
                    agent: 0,
                },
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::rgb(0.15, 0.15, 1.0),
                        custom_size: Some(Vec2::new(10.0, 10.0)),
                        ..default()
                    },
                    transform: Transform {
                        translation: cam
                            .viewport_to_world_2d(cam_t, pos)
                            .unwrap_or(Vec2::ZERO)
                            .extend(0.0),
                        ..default()
                    },
                    ..default()
                },
            ));
        }
    }
}

fn move_to(
    tree: Res<NNTree>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<MoveTowards>>,
) {
    for mut transform in &mut query {
        if let Some(nearest) = tree.nearest_neighbour(transform.translation) {
            let towards = nearest.0 - transform.translation;
            transform.translation += towards.normalize() * time.delta_seconds() * 64.0;
        }
    }
}
*/