mod character_controller;
mod collider_divider;

use bevy::{
    window::{WindowTheme, WindowMode, PresentMode, PrimaryWindow, CursorGrabMode, WindowResized},
    winit::WinitWindows,
    prelude::*,
    diagnostic::LogDiagnosticsPlugin,
    core_pipeline::{bloom::BloomSettings, tonemapping::Tonemapping, motion_blur::{MotionBlur, MotionBlurBundle}, auto_exposure::{AutoExposurePlugin, AutoExposureSettings}, dof::{DepthOfFieldMode, DepthOfFieldSettings}},
    render::{camera::Viewport, view::RenderLayers},
    asset::LoadState,
    pbr::{VolumetricFogSettings, VolumetricLight, ShadowFilteringMethod, CascadeShadowConfigBuilder, NotShadowCaster},
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use avian3d::{math::*, prelude::*};
use winit::window::Icon;
use character_controller::*;

const CHUNK_SIZE: f32 = 30.0;

#[derive(Resource)]
struct Keybinds {
    pause: KeyCode,    // Default Esc
}

#[derive(Component)]
struct Subcollider {
    colliders: Vec<(collider_divider::ChunkPos, Collider)>,
    chunk_size: f32,
    active_colliders: Vec<Entity>,
}

impl Subcollider {
    pub fn new(colliders: Vec<(collider_divider::ChunkPos, Collider)>, chunk_size: f32) -> Self {
        Self {
            colliders: colliders,
            chunk_size: chunk_size,
            active_colliders: Vec::new()
        }
    }
}

fn setup_camera(mut commands: Commands, /*temporary */mut meshes: ResMut<Assets<Mesh>>,) {
    let mut camera_pos = Transform::from_xyz(10.0, 10.0, 16.0);
    let cube_mesh = meshes.add(Cuboid::default());    
    camera_pos.look_at(Vec3::ZERO, Vec3::Y);
    commands.spawn((
        PlayerRigidbody,
        ColliderDensity(985.0), 
        // `SpatialBundle` will get removed in Bevy 0.15
        SpatialBundle {
            transform: camera_pos,
            ..SpatialBundle::default()
        },
        CharacterControllerBundle::new(Collider::capsule(0.11, 1.6)).with_movement(
            80.0,
            0.92,
            22.0,
            (30.0 as Scalar).to_radians()
        ),
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
        Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
        ChunkLoader,
        GravityScale(4.0),
    )).with_children(|b| {b.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 0.8, 0.0),
            projection: Projection::Perspective(PerspectiveProjection {
                far: 10000.0, // change the maximum render distance
                ..default()
            }),            
            camera: Camera {
                order: 0,
                //clear_color: ClearColorConfig::None,
                ..default()
            },  
            tonemapping: Tonemapping::AcesFitted,
            ..default()
        },
        BloomSettings::NATURAL,
        PlayerCamera,
        ViewBobTimer {
            timer: 0.0
        },
        VolumetricFogSettings {
            // This value is explicitly set to 0 since we have no environment map light
            ambient_intensity: 0.0,
            ..default()
        },
        MotionBlurBundle {
            motion_blur: MotionBlur {
                shutter_angle: 1.0,
                samples: 2,
                #[cfg(all(feature = "webgl2", target_arch = "wasm32", not(feature = "webgpu")))]
                _webgl2_padding: Default::default(),
            },
            ..default()
        },
        FogSettings {
            color: Color::srgba(0.35, 0.48, 0.66, 1.0),
            directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.5),
            directional_light_exponent: 30.0,
            falloff: FogFalloff::from_visibility_colors(
                1000.0, // distance in world units up to which objects retain visibility (>= 5% contrast)
                Color::srgb(0.35, 0.5, 0.66), // atmospheric extinction color (after light is lost due to absorption by atmospheric particles)
                Color::srgb(0.8, 0.844, 1.0), // atmospheric inscattering color (light gained due to scattering from the sun)
            ),
        },
        AutoExposureSettings {
            ..default()
        },
        DepthOfFieldSettings {
            mode: DepthOfFieldMode::Bokeh,
            focal_distance: 40.0,
            aperture_f_stops: 0.19, // calculated from human eye
            ..default()
        },
        ShadowFilteringMethod::Gaussian,
        // default render layer is 0
        RenderLayers::from_layers(&[0, 1])
    ));});

    let mut minimap_transform = Transform::from_xyz(10.0, 212.0, 16.0);
    minimap_transform.look_at(Vec3::new(10.0, 12.0, 16.0), Vec3::Y);
    commands.spawn((
        Camera3dBundle {
            transform: minimap_transform,
            projection: Projection::Perspective(PerspectiveProjection {
                far: 10000.0, // change the maximum render distance
                ..default()
            }),          
            camera: Camera {
                order: 2,
                //clear_color: ClearColorConfig::None,
                ..default()
            },  
            tonemapping: Tonemapping::AcesFitted,
            ..default()
        },
        MinimapCamera,
        RenderLayers::from_layers(&[0, 2])
));
    commands.spawn((
        PbrBundle {
            mesh: cube_mesh.clone(),
            transform: Transform::from_xyz(12.0, 10.0, 16.0)
                .with_scale(Vec3::splat(1.6 as f32)),
            ..default()
        },
        RigidBody::Dynamic,
        Collider::cuboid(1.0, 1.0, 1.0),
        ChunkLoader
    ));
}

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct PlayerRigidbody;

#[derive(Component)]
struct ChunkLoader;

#[derive(Component)]
struct MinimapCamera;

#[derive(Component)]
struct ViewBobTimer {
    timer: f32,
}

#[derive(Resource, Default)]
struct AssetLoadingTracker(Vec<UntypedHandle>);

#[derive(Resource)]
struct AssetsCache {
    map_scene: Handle<Scene>,
    map_gltf: Handle<bevy::gltf::Gltf>
}

fn load_assets(
    server: Res<AssetServer>,
    mut tracker: ResMut<AssetLoadingTracker>,
    mut commands: Commands
) {
    // TODO: investigate if theres a way to get this from the gltf file
    let map_scene = server.load("earth map old.glb#Scene0");
    tracker.0.push(map_scene.clone().into());

    let map_gltf = server.load("earth map old.glb");
    tracker.0.push(map_gltf.clone().into());

    commands.insert_resource(AssetsCache{
        map_scene: map_scene,
        map_gltf: map_gltf
    })
}

fn check_assets_ready(
    server: Res<AssetServer>,
    loading: Res<AssetLoadingTracker>,
    mut state: ResMut<NextState<AssetState>>
) {
    let mut iterator = loading.0.iter().map(|h| h.id());

    state.set(AssetState::Loaded);

    while let Some(asset) = iterator.next() {
        match server.get_load_state(asset) {
            Some(LoadState::Failed(_)) | None | Some(LoadState::NotLoaded) => {
                panic!("Asset loading failed"); // code stops here so no need to break
            },
            Some(LoadState::Loaded) => {
                continue;
            },
            Some(LoadState::Loading) => {
                state.set(AssetState::Loading);
                // Break loop - we are still loading assets
                return;
            }
        }
    }
}

// TODO: migrate to state machine
#[derive(States, Default, PartialEq, Debug, Eq, Hash, Clone, Copy)]
enum GameState {
    Paused,
    #[default]
    Playing
}

fn move_camera(
    (rigidbody, mut camera, mut primary_window, mut minimap_camera_query):         (
            // ugh with/without hell
            Query<(&Transform, &LinearVelocity), (With<PlayerRigidbody>, Without<PlayerCamera>)>,
            Query<(&mut Transform, &mut ViewBobTimer), (With<PlayerCamera>, Without<PlayerRigidbody>)>,
            Query<&mut Window, With<PrimaryWindow>>,
            Query<&mut Transform, (With<MinimapCamera>, Without<PlayerCamera>, Without<PlayerRigidbody>)>
        ),
    (keys, mut paused, game_state, keybinds): (Res<ButtonInput<KeyCode>>, ResMut<NextState<GameState>>, Res<State<GameState>>, Res<Keybinds>)
) {
    let mut window = primary_window.get_single_mut().unwrap();

    if *game_state.get() == GameState::Playing  {
        for (mut transform, mut view_bob_timer) in camera.iter_mut() { 
            let Ok((rigidbody_transform, rigidbody_velocity)) = rigidbody.get_single() else {todo!()};
            // TODO: migrate the following code into other functions

            for mut minimaptransform in minimap_camera_query.iter_mut() {
                minimaptransform.translation.x = rigidbody_transform.translation.x;
                minimaptransform.translation.y = rigidbody_transform.translation.y + 500.0;
                minimaptransform.translation.z = rigidbody_transform.translation.z;
            }

            //transform.translation.y = rigidbody_transform.translation.y + (view_bob_timer.timer * 0.2).sin() * 0.2 + 0.2;

            view_bob_timer.timer += rigidbody_velocity.length();

            // prevent timer from overflowing u32 capacity
            if view_bob_timer.timer.sin() == 0.0 || rigidbody_velocity.length() == 0.0 {
                view_bob_timer.timer = 0.0;
            }
        }
    }

    if keys.just_pressed(keybinds.pause) { 
        match game_state.get() {
            GameState::Paused => {
                window.cursor.grab_mode = CursorGrabMode::Confined;
                window.cursor.visible = false;
                paused.set(GameState::Playing);
            }
            _ => {
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
                paused.set(GameState::Paused);
            }
        }
    }
}

fn resize_minimap(
    mut minimap_query: Query<&mut Camera, With<MinimapCamera>>,
    mut resize_events: EventReader<WindowResized>,
    windows: Query<&Window>
) {
    for event in resize_events.read() {
        let window = windows.get(event.window).unwrap();
        let mut minimap = minimap_query.get_single_mut().unwrap();
        minimap.viewport = Some(Viewport {
            physical_position: UVec2::new(25, 25),
            physical_size: UVec2::new((window.width() / 7.0) as u32, (window.width() / 7.0) as u32),
            ..default()
        });
        minimap.order = 2;
    }
}

fn setup_minimap(
    mut minimap_query: Query<&mut Camera, With<MinimapCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>
) {
    let window = windows.get_single().unwrap();
    let mut minimap = minimap_query.get_single_mut().unwrap();
    minimap.viewport = Some(Viewport {
        physical_position: UVec2::new(25, 25),
        physical_size: UVec2::new((window.width() / 7.0) as u32, (window.width() / 7.0) as u32),
        ..default()
    });
    minimap.order = 2;
}

fn update_minimap(
    mut gizmos: Gizmos,
    player_query: Query<&GlobalTransform, With<PlayerRigidbody>>,
) {
    let global_transform = player_query.get_single().unwrap();
    let transform = global_transform.compute_transform();

    gizmos.primitive_3d(
        &Polyline3d::<3>::new(vec!(
            Vec3::new(-0.5, 0.0, 0.5),
            Vec3::new(0.0, 0.0, -0.5),
            Vec3::new(0.5, 0.0, 0.5),
        )),
        Vec3::new(
            transform.translation.x,
            transform.translation.y + 480.0,
            transform.translation.z
        ),
        Quat::from_rotation_y(transform.rotation.to_euler(EulerRot::YXZ).0),
        Color::srgb(0.0, 1.0, 0.0)
    );
}

fn select_subcollider(
    mut divided_colliders: Query<(Entity, &mut Subcollider, &Transform)>,
    loader_query: Query<&Transform, With<ChunkLoader>>, 
    mut commands: Commands
) {
    // nested loop hell
    for (entity, mut subcolliders, map_transform) in divided_colliders.iter_mut() {
        for entity in subcolliders.active_colliders.clone() {
            commands.entity(entity).despawn();
        }

        subcolliders.active_colliders.clear();

        for player_pos in loader_query.iter(){
            for subcollider in subcolliders.colliders.clone() {
                // i have no clue why we need to multiply by 1.725, but it works
                let scaled_chunk_size = subcolliders.chunk_size * map_transform.scale.length() * 1.725;
                let relative_player_pos = player_pos.translation - map_transform.translation;

                let player_pos_rounded = collider_divider::ChunkPos::from_vertex(&collider_divider::Vertex::from(relative_player_pos), &scaled_chunk_size);
            
                if player_pos_rounded == subcollider.0 {
                    let entity_subcollider = commands.spawn(subcollider.1).id();
                    subcolliders.active_colliders.push(entity_subcollider);
                    commands.entity(entity).add_child(entity_subcollider);
                } 
            }
        }
        
    }
}

fn spawn_map(
    mut commands: Commands, 
    handles: Res<AssetsCache>, 
    gltf: Res<Assets<bevy::gltf::Gltf>>, 
    mut gltf_assets: ResMut<Assets<bevy::gltf::GltfMesh>>, 
    mut assets: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>
) {
    // skybox
    commands.spawn((PbrBundle {
        mesh: assets.add(Cuboid::new(1.0, 1.0, 1.0)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.35, 0.48, 0.66, 1.0).into(),
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        transform: Transform::from_scale(Vec3::splat(1_000_000.0)),
        ..default()
    }, NotShadowCaster));

    let scene = gltf.get(&handles.map_gltf).unwrap();
    let mut meshes = Vec::new();

    for mut primitive in gltf_assets.get_mut(&scene.meshes[0]).unwrap().primitives.clone() {
        meshes.push(assets.get_mut(&mut primitive.mesh).unwrap().clone());
    }

    commands.spawn((
        SceneBundle {
            transform: Transform::from_xyz(0.0, -800.0, 0.0).with_scale(Vec3::splat(20.0)),
            scene: handles.map_scene.clone(),
            ..default()
        },
        Subcollider::new(
            meshes.into_iter().fold(Vec::new(), |mut accumulator, mesh| {
                accumulator.extend(collider_divider::split_subcolliders(&mesh, CHUNK_SIZE));
                accumulator
            }),
            10.0,
        ),
        RigidBody::Static,
        CollisionMargin(0.4),
    ))
    .with_children(|children| {
        children.spawn(SpotLightBundle {
            spot_light: SpotLight {
                radius: 10.0,
                color: Color::srgb(1.0, 1.0, 1.0),
                intensity: 10000000000000000.0,
                range: 1000000000.0,
                outer_angle: 0.8,
                shadows_enabled: true,
                ..default()
            },
            transform: Transform::from_xyz(0.0, 75.0, 10000.0),
            ..default()
        })
        .insert(VolumetricLight)
        .insert(CascadeShadowConfigBuilder {
            first_cascade_far_bound: 0.3,
            maximum_distance: 3.0,
            ..default()
        }.build());

        children.spawn((ColliderConstructor::Cuboid {
                x_length: 200.0, 
                y_length: 200.0, 
                z_length: 1.0,
            }, Transform::from_xyz(-50.0, 0.0, 50.0)));

        children.spawn((ColliderConstructor::Cuboid {
                x_length: 200.0, 
                y_length: 200.0, 
                z_length: 1.0,
            }, Transform::from_xyz(-50.0, 0.0, -50.0)));

        children.spawn((ColliderConstructor::Cuboid {
                x_length: 1.0, 
                y_length: 200.0, 
                z_length: 200.0,
            }, Transform::from_xyz(50.0, 0.0, -50.0)));

        children.spawn((ColliderConstructor::Cuboid {
                x_length: 1.0, 
                y_length: 200.0, 
                z_length: 200.0,
            }, Transform::from_xyz(-50.0, 0.0, 50.0)));
    });
}

fn confine_mouse(mut primary_window: Query<&mut Window, With<PrimaryWindow>>) {
    let mut window = primary_window.get_single_mut().unwrap();
    window.cursor.grab_mode = CursorGrabMode::Confined;
    window.cursor.visible = false;
}


fn set_window_icon(
    // we have to use `NonSend` here
    windows: NonSend<WinitWindows>,
) {
    // here we use the `image` crate to load our icon data from a png file
    // this is not a very bevy-native solution, but it will do
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open("assets/app-icon.png")
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    let icon = Icon::from_rgba(icon_rgba, icon_width, icon_height).unwrap();

    // do it for all windows
    for window in windows.windows.values() {
        window.set_window_icon(Some(icon.clone()));
    }
}

//TODO: implement this
/*fn update_time(
    mut time: ResMut<GameTime>,
    sun: Query<Transform, With<Sun>>
)*/

#[derive(States, Debug, Clone, PartialEq, Eq, Hash)]
enum AssetState {
    Loading,
    Loaded
}

fn main() {
    let mut app = App::new();

    app.add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "The Voyage of the Abeona".into(),
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
        //.add_plugins(WorldInspectorPlugin::new())
        .add_plugins(AutoExposurePlugin)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(CharacterControllerPlugin);

    //app.add_plugins(PhysicsDebugPlugin::default());
      /*  .insert_gizmo_config(PhysicsGizmos::colliders(bevy::color::palettes::css::ORANGE.into()), GizmoConfig::default());
      */ 
    app.insert_state(AssetState::Loading)
        // TODO: save player preferences
        .init_state::<GameState>()
        .init_resource::<AssetLoadingTracker>()
        .insert_resource(Keybinds { pause: KeyCode::Escape });

    app.add_systems(PreStartup, load_assets)
        .add_systems(PreStartup, setup_camera)
        .add_systems(PreStartup, set_window_icon)
        .add_systems(Startup, setup_minimap)
        .add_systems(Startup, confine_mouse)
        .add_systems(OnEnter(AssetState::Loaded), spawn_map)
        .add_systems(Update, check_assets_ready.run_if(in_state(AssetState::Loading)))
        .add_systems(PreUpdate, select_subcollider.run_if(in_state(AssetState::Loaded)))
        .add_systems(Update, move_camera.run_if(in_state(AssetState::Loaded)))
        .add_systems(Update, update_minimap.run_if(in_state(AssetState::Loaded)))
        .add_systems(PostUpdate, resize_minimap.run_if(in_state(AssetState::Loaded)))
        .run();
}
