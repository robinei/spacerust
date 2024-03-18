//! Load a cubemap texture onto a cube like a skybox and cycle through different compressed texture formats

#[path = "./camera_controller.rs"]
mod camera_controller;

#[path = "./spatial_index.rs"]
mod spatial_index;

use bevy::{
    asset::LoadState, core_pipeline::Skybox, input::common_conditions::input_just_pressed, math::Vec3A, prelude::*, render::{
        primitives::Aabb, render_resource::{TextureViewDescriptor, TextureViewDimension}
    }, window::{close_on_esc, PrimaryWindow, WindowMode}
};
use camera_controller::{CameraController, CameraControllerPlugin};
use spatial_index::*;
use std::f32::consts::PI;
use rand::prelude::*;


#[derive(Resource)]
struct SkyboxResource {
    is_loaded: bool,
    image_handle: Handle<Image>,
}

#[derive(Component)]
struct ModelEntity;

#[derive(Resource)]
struct CursorPosition {
    position: Vec3,
}



fn limit(v: Vec3, len: f32) -> Vec3 {
    if v.length() > len {
        return v.normalize() * len;
    }
    return v;
}


#[derive(Component)]
struct Velocity {
    velocity: Vec3,
    max_velocity: f32,
    max_force: f32,
    turn_speed: f32,
}

#[derive(Component)]
struct Acceleration {
    acceleration: Vec3,
}



const BOID_RADIUS: f32 = 20.0;

struct Boid<'a> {
    transform: &'a Transform,
    velocity: &'a Velocity,

    sep_sum: Vec3,
    sep_count: i32,

    align_sum: Vec3,
    align_count: i32,

    cohesion_sum: Vec3,
    cohesion_count: i32,
}

impl<'a> Boid<'a> {
    fn new(transform: &'a Transform, velocity: &'a Velocity) -> Self {
        Self {
            transform,
            velocity,
            sep_sum: Vec3::ZERO,
            sep_count: 0,
            align_sum: Vec3::ZERO,
            align_count: 0,
            cohesion_sum: Vec3::ZERO,
            cohesion_count: 0,
        }
    }

    fn add_other(&mut self, transform: &Transform, velocity: &Velocity) {
        let delta = self.transform.translation - transform.translation;
        let dist = delta.length();
        if dist > BOID_RADIUS || dist < 0.001 {
            return
        }

        if dist < 10.0 {
            self.sep_sum += delta.normalize() / dist;
            self.sep_count += 1;
        }
        
        if dist < 20.0 {
            self.align_sum += velocity.velocity;
            self.align_count += 1;
        }

        if dist < 20.0 {
            self.cohesion_sum += transform.translation;
            self.cohesion_count += 1;
        }
    }

    fn get_acceleration(&self) -> Vec3 {
        let mut sum = Vec3::ZERO;
        if self.sep_count > 0 {
            sum += self.steer(self.sep_sum / (self.sep_count as f32));
        }
        if self.align_count > 0 {
            sum += self.steer(self.align_sum / (self.align_count as f32));
        }
        if self.cohesion_count > 0 {
            sum += self.seek(self.cohesion_sum / (self.cohesion_count as f32));
        }
        sum
    }

    fn seek(&self, target: Vec3) -> Vec3 {
        return self.steer(target - self.transform.translation);
    }

    fn steer(&self, dir: Vec3) -> Vec3 {
        let len = dir.length();
        if len < 0.000001 {
            return Vec3::ZERO;
        }
        let adjusted_dir = dir * (self.velocity.max_velocity / len);
        return limit(adjusted_dir - self.velocity.velocity, self.velocity.max_force);
    }

    fn arrive(&self, target: Vec3) -> Vec3 {
        let brakelimit = 50.0;
        let mut desired = target - self.transform.translation;
        let len = desired.length();
        if len < 0.000001 {
            return Vec3::ZERO;
        }
        desired /= len;
        if len < brakelimit {
            desired *= (len / brakelimit) * self.velocity.max_velocity;
        } else {
            desired *= self.velocity.max_velocity;
        }
        return limit(desired, self.velocity.max_force);
    }
}


fn calc_acceleration(
    cursor: Res<CursorPosition>,
    index: Res<SpatialIndex>,
    mut query: Query<(Entity, &Transform, &Velocity, &mut Acceleration)>,
    lookup_query: Query<(&Transform, &Velocity)>,
) {
    for (entity1, trans1, vel1, mut acc) in &mut query {
        let mut boid = Boid::new(trans1, vel1);
        index.query(trans1.translation, BOID_RADIUS, |entity2| {
            if entity1 != entity2 {
                if let Ok((trans2, vel2)) = lookup_query.get(entity2) {
                    boid.add_other(trans2, vel2);
                }
            }
        });
        acc.acceleration = boid.get_acceleration() + boid.steer(cursor.position - trans1.translation);
    }
}

fn move_by_velocity(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Velocity, &Acceleration)>,
    mut gizmos: Gizmos
) {
    gizmos.arrow(Vec3::ZERO, Vec3::X * 20.0, Color::RED);
    gizmos.arrow(Vec3::ZERO, Vec3::Y * 20.0, Color::GREEN);
    gizmos.arrow(Vec3::ZERO, Vec3::Z * 20.0, Color::BLUE);

    for (mut transform, mut vel, acc) in &mut query {
        vel.velocity += acc.acceleration * time.delta_seconds();
        vel.velocity = limit(vel.velocity, vel.max_velocity);
        vel.velocity.y = 0.0;

        transform.translation += vel.velocity * time.delta_seconds();
        transform.translation.y = 0.0;

        let target = transform.looking_to(-vel.velocity, Vec3::Y);
        transform.rotation = transform.rotation.lerp(target.rotation, vel.turn_speed * time.delta_seconds());

        //gizmos.arrow(transform.translation, transform.translation + vel.velocity, Color::WHITE);
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
                calc_acceleration,
                move_by_velocity.after(calc_acceleration),
                update_cell_association,
                update_spatial_index.after(update_cell_association),
                adjust_by_aabb,
                skybox_system,
                test_spatial_index,
                update_cursor_ground_plane_position,
                close_on_esc
            ),
        )
        .run();
}



fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 32000.0,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 20.0, 0.0)
            .with_rotation(Quat::from_rotation_x(-PI / 4.)),
        ..default()
    });
    
    let image_handle = asset_server.load("space_cubemap.png");
    commands.insert_resource(SkyboxResource {
        is_loaded: false,
        image_handle: image_handle.clone(),
    });

    commands.insert_resource(SpatialIndex::new());

    commands.insert_resource(CursorPosition {
        position: Vec3::ZERO,
    });

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
        CellAssociation::new(),
        SpatialBundle {
            transform: Transform::from_translation(position),
            ..default()
        },
        Acceleration {
            acceleration: Vec3::ZERO,
        },
        Velocity {
            velocity,
            max_velocity: 10.0,
            max_force: 1.0,
            turn_speed: 1.0,
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

fn update_cursor_ground_plane_position(
    mut cursor: ResMut<CursorPosition>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform)>,
) {
    let (camera, camera_transform) = q_camera.single();
    let window = q_window.single();
    let Some(cursor_position) = window.cursor_position() else {
        return;
    };
    let plane = Plane3d::new(Vec3::Y);
    let Some(ray) = camera.viewport_to_world(camera_transform, cursor_position) else {
        return;
    };
    let Some(distance) = ray.intersect_plane(Vec3::ZERO, plane) else {
        return;
    };
    cursor.position = ray.get_point(distance);
}
