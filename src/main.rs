use bevy::{
    window::{WindowTheme, WindowMode, PresentMode, PrimaryWindow, CursorGrabMode},
    prelude::*,
    diagnostic::{LogDiagnosticsPlugin},
    winit::WinitWindows,
    input::{prelude::*, mouse::MouseMotion},
    ecs::event::ManualEventReader
};
use winit::window::Icon;

fn set_window_icon(
    windows: NonSend<WinitWindows>,
) {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open("assets/app-icon.png")
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    let icon = Icon::from_rgba(icon_rgba, icon_width, icon_height).unwrap();

    for window in windows.windows.values() {
        window.set_window_icon(Some(icon.clone()));
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(10.0, 12.0, 16.0),
            ..default()
        },
        PlayerCamera,
        Velocity {v: Vec3::new(0.0, 0.0, 0.0)},
        Position{v: Vec3::new(10.0, 12.0, 16.0)},
        ViewBobTimer {
            timer: 0.0
        }
    ));
}

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct Collidable {
    // use Collider::from_bevy_mesh()
    collider: Collider
}

#[derive(Component)]
struct Velocity {
    v: Vec3,
}

// need a position vector so that we can have viewbob and such
#[derive(Component)]
struct Position {
    v: Vec3,
}

#[derive(Component)]
struct ViewBobTimer {
    timer: f32,
}

#[derive(Resource)]
struct Keybinds {
    left: KeyCode,     // Default A
    right: KeyCode,    // Default D
    forward: KeyCode,  // Default W
    backward: KeyCode, // Default S
    pause: KeyCode,    // Default Esc
    sensitivity: f32   // Default 0.00006
}

#[derive(Resource, Default, PartialEq)]
enum GameState {
    #[default]
    Paused,
    Playing
}

/// Keeps track of mouse motion events, pitch, and yaw
#[derive(Resource, Default)]
struct InputState {
    reader_motion: ManualEventReader<MouseMotion>,
}

// TODO: implement Mesh component
fn move_camera(
    (mut query, mut primary_window, collidables): (Query<(&mut Transform, &mut Velocity, &mut Position, &mut ViewBobTimer, &PlayerCamera, &Collidable)>, Query<&mut Window, With<PrimaryWindow>>, Query<&Collidable>),
    (keys, mut paused, time, keybinds, mut state, motion): (Res<ButtonInput<KeyCode>>, ResMut<GameState>, Res<Time>, Res<Keybinds>, ResMut<InputState>, Res<Events<MouseMotion>>,)
) {
    let mut window = primary_window.get_single_mut().unwrap();

    if *paused == GameState::Paused  {
        for (mut transform, mut velocity, mut position, mut viewBobTimer, _, player_collider) in query.iter_mut() {
            let local_z = transform.local_z();
            if keys.pressed(keybinds.left) {
                velocity.v += Vec3::new(local_z.z, 0., -local_z.x) * time.delta_seconds() * -10.0;
            }
            if keys.pressed(keybinds.right) {
                velocity.v += Vec3::new(local_z.z, 0., -local_z.x) * time.delta_seconds() * 10.0;
            }
            if keys.pressed(keybinds.forward) {
                velocity.v += -Vec3::new(local_z.x, 0., local_z.z) * time.delta_seconds() * 10.0;
            }
            if keys.pressed(keybinds.backward) {
                velocity.v += -Vec3::new(local_z.x, 0., local_z.z) * time.delta_seconds() * -10.0;
            }

            // friction
            velocity.v.x *= 0.75;
            velocity.v.z *= 0.75;

            // gravity
            velocity.v.y -= 10.0 * time.delta_seconds();

            position.v.x += velocity.v.x;
            // TODO: collision check
            for collidable in collidables {
                if 
            }
            position.v.y += velocity.v.y;
            // TODO: collision check
            position.v.z += velocity.v.z;
            // TODO: collision check

            transform.translation = position.v;

            transform.translation.y = position.v.y + (viewBobTimer.timer * 0.5).sin() * 0.2;

            viewBobTimer.timer += velocity.v.length();

            // prevent timer from overflowing capacity
            if viewBobTimer.timer.sin() == 0.0 || velocity.v.length() == 0.0 {
                viewBobTimer.timer = 0.0;
            }

            for ev in state.reader_motion.read(&motion) {
                let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
                match window.cursor.grab_mode {
                    CursorGrabMode::None => (),
                    _ => {
                        // Using smallest of height or width ensures equal vertical and horizontal sensitivity
                        let window_scale = window.height().min(window.width());
                        pitch -= (keybinds.sensitivity * ev.delta.y * window_scale).to_radians();
                        yaw -= (keybinds.sensitivity * ev.delta.x * window_scale).to_radians();
                    }
                }
        
                pitch = pitch.clamp(-1.54, 1.54);
        
                // Order is important to prevent unintended roll
                transform.rotation =
                    Quat::from_axis_angle(Vec3::Y, yaw) * Quat::from_axis_angle(Vec3::X, pitch);
            }
        }
    }

    if keys.just_pressed(keybinds.pause) {
        
        match *paused {
            GameState::Paused => {
                window.cursor.grab_mode = CursorGrabMode::Confined;
                window.cursor.visible = false;
                *paused = GameState::Playing;
            }
            _ => {
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
                *paused = GameState::Paused;
            }
        }
        
    }
}

fn spawn_map(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(bevy::math::primitives::Cuboid::new(100.0, 1.0, 100.0)),
            material: materials.add(Color::WHITE),
            ..default()
        },
        Collidable
    )).with_children(|children| {
        children.spawn(PointLightBundle {
            point_light: PointLight {
                radius: 9000.0,
                color: Color::rgb(1.0, 1.0, 1.0),
                intensity: 100000.0,
                range: 1000.0,
                ..default()
            },
            transform: Transform::from_xyz(0.0, 5.0, 0.0),
            ..default()
        });
    });
}

fn confine_mouse(mut primary_window: Query<&mut Window, With<PrimaryWindow>>) {
    let mut window = primary_window.get_single_mut().unwrap();
    window.cursor.grab_mode = CursorGrabMode::Confined;
    window.cursor.visible = false;
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Le Voyage D'Abeon".into(),
                    name: Some("Voyage of the Abeona".into()),
                    mode: WindowMode::BorderlessFullscreen,
                    present_mode: PresentMode::AutoVsync,
                    window_theme: Some(WindowTheme::Dark),
                    enabled_buttons: bevy::window::EnabledButtons::default(),
                    visible: true,
                    ..default()
                }),
                ..default()
            }),
            LogDiagnosticsPlugin::default()
        ))
        // TODO: save player preferences
        .insert_resource(Keybinds { 
            left: KeyCode::KeyA,
            right: KeyCode::KeyD,
            forward: KeyCode::KeyW,
            backward: KeyCode::KeyS,
            pause: KeyCode::Escape,
            sensitivity: 0.00006
        })
        .init_resource::<GameState>()
        .init_resource::<InputState>()
        .add_systems(PreStartup, set_window_icon)
        .add_systems(Startup, (setup_camera, spawn_map))
        .add_systems(Startup, confine_mouse)
        .add_systems(Update, move_camera)
        .run();
}
