use avian3d::{math::*, prelude::*};
use bevy::{ecs::query::Has, prelude::*, window::PrimaryWindow, input::mouse::MouseMotion, ecs::event::ManualEventReader};

#[derive(Resource)]
struct MovementKeybinds {
    left: KeyCode,     // Default A
    right: KeyCode,    // Default D
    forward: KeyCode,  // Default W
    backward: KeyCode, // Default S
    sprint: KeyCode,   // Default Shift
    sensitivity: f32   // Default 0.00006
}

pub struct CharacterControllerPlugin;

impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MovementAction>().add_systems(
            Update,
            (
                keyboard_input,
                gamepad_input,
                mouse_input,
                update_grounded,
                movement,
                apply_movement_damping,
            )
                .chain(),
        );
        app.init_resource::<Yaw>();
        app.init_resource::<CameraRotation>();
        app.init_resource::<State>();
        app.insert_resource(MovementKeybinds { 
            left: KeyCode::KeyA,
            right: KeyCode::KeyD,
            forward: KeyCode::KeyW,
            backward: KeyCode::KeyS,
            sprint: KeyCode::ShiftLeft,
            sensitivity: 0.0006
        });
    }
}

/// An event sent for a movement input action.
#[derive(Event)]
pub enum MovementAction {
    Move(Vector2),
    Rotate(Quat),
    Jump,
}

/// A marker component indicating that an entity is using a character controller.
#[derive(Component)]
pub struct CharacterController;

/// A marker component indicating that an entity is on the ground.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Grounded;
/// The acceleration used for character movement.
#[derive(Component)]
pub struct MovementAcceleration(Scalar);

/// The damping factor used for slowing down movement.
#[derive(Component)]
pub struct MovementDampingFactor(Scalar);

/// The strength of a jump.
#[derive(Component)]
pub struct JumpImpulse(Scalar);

/// The maximum angle a slope can have for a character controller
/// to be able to climb and jump. If the slope is steeper than this angle,
/// the character will slide down.
#[derive(Component)]
pub struct MaxSlopeAngle(Scalar);

/// A bundle that contains the components needed for a basic
/// kinematic character controller.
#[derive(Bundle)]
pub struct CharacterControllerBundle {
    character_controller: CharacterController,
    rigid_body: RigidBody,
    collider: Collider,
    ground_caster: ShapeCaster,
    locked_axes: LockedAxes,
    movement: MovementBundle,
}

/// A bundle that contains components for character movement.
#[derive(Bundle)]
pub struct MovementBundle {
    acceleration: MovementAcceleration,
    damping: MovementDampingFactor,
    jump_impulse: JumpImpulse,
    max_slope_angle: MaxSlopeAngle,
}

impl MovementBundle {
    pub const fn new(
        acceleration: Scalar,
        damping: Scalar,
        jump_impulse: Scalar,
        max_slope_angle: Scalar,
    ) -> Self {
        Self {
            acceleration: MovementAcceleration(acceleration),
            damping: MovementDampingFactor(damping),
            jump_impulse: JumpImpulse(jump_impulse),
            max_slope_angle: MaxSlopeAngle(max_slope_angle),
        }
    }
}

impl Default for MovementBundle {
    fn default() -> Self {
        Self::new(30.0, 0.9, 7.0, PI * 0.45)
    }
}

impl CharacterControllerBundle {
    pub fn new(collider: Collider) -> Self {
        // Create shape caster as a slightly smaller version of collider
        let mut caster_shape = collider.clone();
        caster_shape.set_scale(Vector::ONE * 0.99, 10);

        Self {
            character_controller: CharacterController,
            rigid_body: RigidBody::Dynamic,
            collider,
            ground_caster: ShapeCaster::new(
                caster_shape,
                Vector::ZERO,
                Quaternion::default(),
                Dir3::NEG_Y,
            )
            .with_max_time_of_impact(0.2),
            locked_axes: LockedAxes::ROTATION_LOCKED,
            movement: MovementBundle::default(),
        }
    }

    pub fn with_movement(
        mut self,
        acceleration: Scalar,
        damping: Scalar,
        jump_impulse: Scalar,
        max_slope_angle: Scalar,
    ) -> Self {
        self.movement = MovementBundle::new(acceleration, damping, jump_impulse, max_slope_angle);
        self
    }
}

/// Sends [`MovementAction`] events based on keyboard input.
fn keyboard_input(
    mut movement_event_writer: EventWriter<MovementAction>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    keybinds: Res<MovementKeybinds>
) {
    let up = keyboard_input.any_pressed([keybinds.forward]);
    let down = keyboard_input.any_pressed([keybinds.backward]);
    let left = keyboard_input.any_pressed([keybinds.left]);
    let right = keyboard_input.any_pressed([keybinds.right]);

    let horizontal = right as i8 - left as i8;
    let vertical = up as i8 - down as i8;
    let mut direction = Vector2::new(horizontal as Scalar, vertical as Scalar).clamp_length_max(1.0);

    if keyboard_input.any_pressed([keybinds.sprint]) {
        direction = Vector2::new(direction.x * 2.0, direction.y * 2.0);
    }

    if direction != Vector2::ZERO {
        movement_event_writer.send(MovementAction::Move(direction));
    }

    if keyboard_input.just_pressed(KeyCode::Space) {
        movement_event_writer.send(MovementAction::Jump);
    }
}

/// Keeps track of camera yaw
#[derive(Resource, Default)]
struct Yaw(f32);

/// Keeps track of camera rotation
#[derive(Resource, Default)]
struct CameraRotation(Quat);

#[derive(Resource, Default)]
struct State{reader_motion: ManualEventReader<MouseMotion>}

fn mouse_input(mut state: ResMut<State>, mut yaw: ResMut<Yaw>, movement_keybinds: Res<MovementKeybinds>, camera_rotation: Res<CameraRotation>, mut movement_event_writer: EventWriter<MovementAction>, mut windows: Query<&mut Window, With<PrimaryWindow>>, mouse_motion: Res<Events<MouseMotion>>) {
    let mut window = windows.get_single_mut().unwrap();

    for ev in state.reader_motion.read(&mouse_motion) {
        let mut pitch;
        (yaw.0, pitch, _) = camera_rotation.0.to_euler(EulerRot::YXZ);
        // Using smallest of height or width ensures equal vertical and horizontal sensitivity
        let window_scale = window.height().min(window.width());
        pitch -= (movement_keybinds.sensitivity * ev.delta.y * window_scale).to_radians();
        yaw.0 -= (movement_keybinds.sensitivity * ev.delta.x * window_scale).to_radians();

        pitch = pitch.clamp(-1.54, 1.54);

        // Order is important to prevent unintended roll
        movement_event_writer.send(MovementAction::Rotate(Quat::from_axis_angle(Vec3::Y, yaw.0) * Quat::from_axis_angle(Vec3::X, pitch)));
    }
}

/// Sends [`MovementAction`] events based on gamepad input.
fn gamepad_input(
    mut movement_event_writer: EventWriter<MovementAction>,
    gamepads: Res<Gamepads>,
    axes: Res<Axis<GamepadAxis>>,
    buttons: Res<ButtonInput<GamepadButton>>,
) {
    for gamepad in gamepads.iter() {
        let axis_lx = GamepadAxis {
            gamepad,
            axis_type: GamepadAxisType::LeftStickX,
        };
        let axis_ly = GamepadAxis {
            gamepad,
            axis_type: GamepadAxisType::LeftStickY,
        };

        if let (Some(x), Some(y)) = (axes.get(axis_lx), axes.get(axis_ly)) {
            movement_event_writer.send(MovementAction::Move(
                Vector2::new(x as Scalar, y as Scalar).clamp_length_max(1.0),
            ));
        }

        let jump_button = GamepadButton {
            gamepad,
            button_type: GamepadButtonType::South,
        };

        if buttons.just_pressed(jump_button) {
            movement_event_writer.send(MovementAction::Jump);
        }
    }
}

/// Updates the [`Grounded`] status for character controllers.
fn update_grounded(
    mut commands: Commands,
    mut query: Query<
        (Entity, &ShapeHits, &Rotation, Option<&MaxSlopeAngle>),
        With<CharacterController>,
    >
) {
    for (entity, hits, rotation, max_slope_angle) in &mut query {
        // The character is grounded if the shape caster has a hit with a normal
        // that isn't too steep.
        let is_grounded = hits.iter().any(|hit| {
            if let Some(angle) = max_slope_angle {
                (rotation * -hit.normal2).angle_between(Vector::Y).abs() <= angle.0
            } else {
                true
            }
        });

        if is_grounded {
            commands.entity(entity).insert(Grounded);
        } else {
            commands.entity(entity).remove::<Grounded>();
        }
    }
}

/// Responds to [`MovementAction`] events and moves character controllers accordingly.
fn movement(
    time: Res<Time>,
    mut movement_event_reader: EventReader<MovementAction>,
    mut camera_rotation: ResMut<CameraRotation>,
    mut controllers: Query<(
        &MovementAcceleration,
        &JumpImpulse,
        &mut LinearVelocity,
        &mut Transform,
        Has<Grounded>,
    )>,
) {
    // Precision is adjusted so that the example works with
    // both the `f32` and `f64` features. Otherwise you don't need this.
    let delta_time = time.delta_seconds_f64().adjust_precision();

    for event in movement_event_reader.read() {
        for (movement_acceleration, jump_impulse, mut linear_velocity, mut transform, is_grounded) in
            &mut controllers
        {
            match event {
                MovementAction::Move(direction) => {
                    linear_velocity.x -= direction.y * movement_acceleration.0 * delta_time * (transform.rotation.to_euler(EulerRot::YXZ).0).sin();
                    linear_velocity.z -= direction.y * movement_acceleration.0 * delta_time * (transform.rotation.to_euler(EulerRot::YXZ).0).cos();

                    linear_velocity.x += direction.x * movement_acceleration.0 * delta_time * (transform.rotation.to_euler(EulerRot::YXZ).0).cos();
                    linear_velocity.z -= direction.x * movement_acceleration.0 * delta_time * (transform.rotation.to_euler(EulerRot::YXZ).0).sin();
                    
                },
                MovementAction::Rotate(rotation) => {
                    camera_rotation.0 = *rotation;
                    transform.rotation = *rotation;
                },
                MovementAction::Jump => {
                    if is_grounded {
                        linear_velocity.y = jump_impulse.0;
                    }
                }
            }
        }
    }
}

/// Slows down movement in the XZ plane.
fn apply_movement_damping(mut query: Query<(&MovementDampingFactor, &mut LinearVelocity)>) {
    for (damping_factor, mut linear_velocity) in &mut query {
        // We could use `LinearDamping`, but we don't want to dampen movement along the Y axis
        linear_velocity.x *= damping_factor.0;
        linear_velocity.z *= damping_factor.0;
    }
}