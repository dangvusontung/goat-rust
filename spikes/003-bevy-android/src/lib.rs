use bevy::prelude::*;
use std::f32::consts::PI;

const WALK_SPEED: f32 = 2.0;
const LIMB_SWING_HZ: f32 = 1.6;
const LIMB_SWING_AMPLITUDE: f32 = 0.9;

#[derive(Component)]
struct Character {
    distance: f32,
}

#[derive(Component)]
enum Limb {
    ArmLeft,
    ArmRight,
    LegLeft,
    LegRight,
}

pub fn run_app() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "spike: bevy-android".into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (walk_along_path, animate_limbs))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20.0, 20.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.25, 0.55, 0.25))),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.4, 6.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));

    let skin = materials.add(Color::srgb(0.95, 0.78, 0.6));
    let shirt = materials.add(Color::srgb(0.15, 0.35, 0.85));
    let pants = materials.add(Color::srgb(0.2, 0.2, 0.25));

    let head_mesh = meshes.add(Cuboid::new(0.5, 0.5, 0.5));
    let torso_mesh = meshes.add(Cuboid::new(0.7, 0.9, 0.4));
    let arm_mesh = meshes.add(Cuboid::new(0.25, 0.75, 0.25));
    let leg_mesh = meshes.add(Cuboid::new(0.3, 0.85, 0.3));

    commands
        .spawn((
            Character { distance: 0.0 },
            Transform::from_xyz(-4.0, 0.0, 0.0),
            Visibility::default(),
        ))
        .with_children(|root| {
            let hip_y = 0.85;

            root.spawn((
                Mesh3d(torso_mesh),
                MeshMaterial3d(shirt),
                Transform::from_xyz(0.0, hip_y + 0.45, 0.0),
            ));

            root.spawn((
                Mesh3d(head_mesh),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, hip_y + 0.9 + 0.3, 0.0),
            ));

            for (limb, side) in [(Limb::ArmLeft, -1.0_f32), (Limb::ArmRight, 1.0_f32)] {
                root.spawn((
                    limb,
                    Transform::from_xyz(side * 0.475, hip_y + 0.9 - 0.375, 0.0),
                ))
                .with_children(|shoulder| {
                    shoulder.spawn((
                        Mesh3d(arm_mesh.clone()),
                        MeshMaterial3d(skin.clone()),
                        Transform::from_xyz(0.0, -0.375, 0.0),
                    ));
                });
            }

            for (limb, side) in [(Limb::LegLeft, -1.0_f32), (Limb::LegRight, 1.0_f32)] {
                root.spawn((limb, Transform::from_xyz(side * 0.2, hip_y, 0.0)))
                    .with_children(|hip| {
                        hip.spawn((
                            Mesh3d(leg_mesh.clone()),
                            MeshMaterial3d(pants.clone()),
                            Transform::from_xyz(0.0, -0.425, 0.0),
                        ));
                    });
            }
        });
}

fn walk_along_path(time: Res<Time>, mut q: Query<(&mut Character, &mut Transform)>) {
    for (mut ch, mut tf) in &mut q {
        ch.distance += WALK_SPEED * time.delta_secs();
        let range = 8.0;
        let t = (ch.distance % (range * 2.0)) / range;
        let (x, facing) = if t < 1.0 {
            (-4.0 + t * range, 0.0)
        } else {
            (4.0 - (t - 1.0) * range, PI)
        };
        tf.translation.x = x;
        tf.rotation = Quat::from_rotation_y(facing);
    }
}

fn animate_limbs(
    characters: Query<&Character>,
    mut limbs: Query<(&Limb, &mut Transform, &ChildOf)>,
) {
    for (limb, mut tf, parent) in &mut limbs {
        let Ok(ch) = characters.get(parent.parent()) else {
            continue;
        };
        let phase = ch.distance * LIMB_SWING_HZ * 2.0 * PI / WALK_SPEED;
        let swing = LIMB_SWING_AMPLITUDE * phase.sin();
        let angle = match limb {
            Limb::ArmLeft => swing,
            Limb::ArmRight => -swing,
            Limb::LegLeft => -swing,
            Limb::LegRight => swing,
        };
        tf.rotation = Quat::from_rotation_x(angle);
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: bevy_android::android_activity::AndroidApp) {
    bevy_android::ANDROID_APP
        .set(app)
        .expect("android_main called twice");
    run_app();
}
