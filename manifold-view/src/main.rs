use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use bevy::{
    asset::RenderAssetUsages,
    input::mouse::{MouseMotion, MouseWheel},
    log::LogPlugin,
    math::primitives::Cylinder,
    mesh::Indices,
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

mod stl_loader {
    use super::*;
    use bevy::asset::{io::Reader, AssetLoader, LoadContext};
    use bevy::reflect::TypePath;

    #[derive(Asset, TypePath, Debug, Clone)]
    pub struct StlMesh {
        pub positions: Vec<[f32; 3]>,
        pub normals: Vec<[f32; 3]>,
        pub indices: Vec<u32>,
    }

    impl StlMesh {
        pub fn to_bevy_mesh(&self) -> Mesh {
            let mut m = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            );
            m.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions.clone());
            m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals.clone());
            m.insert_indices(Indices::U32(self.indices.clone()));
            m
        }
    }

    #[derive(Default)]
    pub struct StlLoader;

    impl AssetLoader for StlLoader {
        type Asset = StlMesh;
        type Settings = ();
        type Error = anyhow::Error;

        async fn load(
            &self,
            reader: &mut dyn Reader,
            _settings: &Self::Settings,
            _load_context: &mut LoadContext<'_>,
        ) -> Result<Self::Asset, Self::Error> {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let mut cursor = std::io::Cursor::new(&bytes);
            let mesh = stl_io::read_stl(&mut cursor)?;

            let positions: Vec<[f32; 3]> =
                mesh.vertices.iter().map(|v| [v[0], v[1], v[2]]).collect();

            let mut indices: Vec<u32> = Vec::with_capacity(mesh.faces.len() * 3);
            for tri in &mesh.faces {
                indices.push(tri.vertices[0] as u32);
                indices.push(tri.vertices[1] as u32);
                indices.push(tri.vertices[2] as u32);
            }

            let mut normal_acc: HashMap<usize, Vec<Vec3>> = HashMap::new();
            for tri in &mesh.faces {
                let i0 = tri.vertices[0] as usize;
                let i1 = tri.vertices[1] as usize;
                let i2 = tri.vertices[2] as usize;
                let p0 = Vec3::from(positions[i0]);
                let p1 = Vec3::from(positions[i1]);
                let p2 = Vec3::from(positions[i2]);
                let n = (p1 - p0).cross(p2 - p0).normalize_or_zero();
                normal_acc.entry(i0).or_default().push(n);
                normal_acc.entry(i1).or_default().push(n);
                normal_acc.entry(i2).or_default().push(n);
            }

            let mut normals: Vec<[f32; 3]> = Vec::with_capacity(positions.len());
            for i in 0..positions.len() {
                let n = normal_acc
                    .get(&i)
                    .map(|ns| {
                        let mut s = Vec3::ZERO;
                        for v in ns {
                            s += *v;
                        }
                        s.normalize_or_zero()
                    })
                    .unwrap_or(Vec3::Y);
                normals.push([n.x, n.y, n.z]);
            }

            Ok(StlMesh {
                positions,
                normals,
                indices,
            })
        }

        fn extensions(&self) -> &[&str] {
            &["stl"]
        }
    }

    pub fn compute_bounds(positions: &[[f32; 3]]) -> (Vec3, Vec3) {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for p in positions {
            let v = Vec3::new(p[0], p[1], p[2]);
            min = min.min(v);
            max = max.max(v);
        }
        (min, max)
    }
}

use stl_loader::{compute_bounds, StlLoader, StlMesh};

const GRID_SQUARE_MM: f32 = 10.0;
const GRID_BLOCK_SIZE: usize = 5;
const GRID_BLOCKS: usize = 5;
const ASSETS_DIR: &str = env!("CARGO_MANIFEST_DIR");

#[derive(Resource, Clone)]
struct CliModelPath(PathBuf);

#[derive(Resource, Default)]
struct DragState {
    active: Option<DragData>,
}

#[derive(Clone)]
struct DragData {
    entity: Entity,
    start_entity_pos: Vec3,
    start_hit_world: Vec3,
}

#[derive(Component, Clone, Copy)]
struct OrbitCamera {
    target: Vec3,
    distance: f32,
    yaw: f32,
    pitch: f32,
}

#[derive(Component, Clone)]
struct StlInstance {
    handle: Handle<StlMesh>,
}

#[derive(Component, Clone, Copy)]
struct ModelBounds {
    min: Vec3,
    max: Vec3,
}

#[derive(Component)]
struct Selected;

#[derive(Component)]
struct ModelReady;

fn main() {
    let stl_path = env::args()
        .nth(1)
        .expect("Usage: manifold-view <path/to/model.stl>");
    let stl_path = PathBuf::from(stl_path);
    let _ = std::fs::create_dir_all("assets/imports");

    App::new()
        .insert_resource(CliModelPath(stl_path))
        .insert_resource(DragState::default())
        .insert_resource(ClearColor(Color::srgb_u8(18, 22, 28)))
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 80.0, // match Forge feel
            affects_lightmapped_meshes: true,
        })
        .add_plugins(DefaultPlugins.set(LogPlugin {
            level: bevy::log::Level::DEBUG,
            filter:
                "wgpu=warn,naga=warn,bevy_render=info,bevy_winit=info,manifold_view=debug".into(),
            custom_layer: |_| None,
            fmt_layer: |_| None,
        }))
        .init_asset::<StlMesh>()
        .init_asset_loader::<StlLoader>()
        .add_systems(Startup, (setup_camera, spawn_axes_arrows, begin_load_model))
        .add_systems(
            Update,
            (
                camera_orbit_controls,
                select_and_start_drag,
                update_drag,
                end_drag,
                stl_instantiate_ready,
                draw_grid_gizmos,
            ),
        )
        .run();
}

fn plate_extent() -> f32 {
    GRID_SQUARE_MM * (GRID_BLOCKS * GRID_BLOCK_SIZE) as f32
}

fn plate_center() -> Vec3 {
    let e = plate_extent();
    Vec3::new(e * 0.5, e * 0.5, 0.0)
}

fn setup_camera(mut commands: Commands) {
    let center = plate_center();
    let distance = (plate_extent() * 1.2).max(150.0);
    let yaw = -45f32.to_radians();
    let pitch = 30f32.to_radians();
    let mut tf = Transform::from_translation(offset_from_spherical(distance, yaw, pitch) + center);
    tf.look_at(center, Vec3::Z);
    info!(
        "Camera: center={:?} distance={:.2} yaw_deg={:.1} pitch_deg={:.1}",
        center,
        distance,
        yaw.to_degrees(),
        pitch.to_degrees()
    );

    commands.spawn((
        Camera3d::default(),
        tf,
        GlobalTransform::default(),
        OrbitCamera {
            target: center,
            distance,
            yaw,
            pitch,
        },
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: false,
            ..Default::default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -45f32.to_radians(),
            0.0,
            45f32.to_radians(),
        )),
        GlobalTransform::default(),
    ));
}

fn spawn_axes_arrows(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let shaft_len = plate_extent() * 0.2;
    let tip_len = shaft_len * 0.15;
    let shaft_r = shaft_len * 0.02;
    let tip_r = shaft_len * 0.05;

    let shaft_mesh = meshes.add(Mesh::from(Cylinder {
        radius: shaft_r,
        half_height: shaft_len * 0.5,
    }));
    let tip_mesh = meshes.add(build_cone_mesh(tip_r, tip_len, 32));

    let red = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.1, 0.1),
        cull_mode: None,
        ..Default::default()
    });
    let green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 1.0, 0.1),
        cull_mode: None,
        ..Default::default()
    });
    let blue = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.5, 1.0),
        cull_mode: None,
        ..Default::default()
    });

    let mut spawn_arrow = |axis: Vec3, color: &Handle<StandardMaterial>| {
        let rot = if axis == Vec3::X {
            Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)
        } else if axis == Vec3::Z {
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
        } else {
            Quat::IDENTITY
        };
        let shaft_translation = Vec3::Y * (shaft_len * 0.5);
        let tip_translation = Vec3::Y * shaft_len;

        commands.spawn((
            Mesh3d(shaft_mesh.clone()),
            MeshMaterial3d(color.clone()),
            Transform::from_rotation(rot).with_translation(rot * shaft_translation),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
        ));
        commands.spawn((
            Mesh3d(tip_mesh.clone()),
            MeshMaterial3d(color.clone()),
            Transform::from_rotation(rot).with_translation(rot * tip_translation),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
        ));
    };

    spawn_arrow(Vec3::X, &red);
    spawn_arrow(Vec3::Y, &green);
    spawn_arrow(Vec3::Z, &blue);
}

fn build_cone_mesh(radius: f32, height: f32, segments: usize) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    positions.push([0.0, height, 0.0]);
    normals.push([0.0, 1.0, 0.0]);

    let base_center_index = 1u32;
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);

    let mut rim_indices: Vec<u32> = Vec::with_capacity(segments);
    for i in 0..segments {
        let th = (i as f32) / (segments as f32) * std::f32::consts::TAU;
        let x = radius * th.cos();
        let z = radius * th.sin();
        positions.push([x, 0.0, z]);
        normals.push([0.0, -1.0, 0.0]);
        rim_indices.push((2 + i) as u32);
    }

    for i in 0..segments {
        let i0 = base_center_index;
        let i1 = rim_indices[i];
        let i2 = rim_indices[(i + 1) % segments];
        indices.extend_from_slice(&[i0, i2, i1]);
    }

    for i in 0..segments {
        let i_rim0 = rim_indices[i] as usize;
        let i_rim1 = rim_indices[(i + 1) % segments] as usize;
        let p_apex = Vec3::new(0.0, height, 0.0);
        let p0 = Vec3::from(positions[i_rim0]);
        let p1 = Vec3::from(positions[i_rim1]);
        let n = (p1 - p_apex).cross(p0 - p_apex).normalize_or_zero();
        let base = positions.len() as u32;
        positions.push([p_apex.x, p_apex.y, p_apex.z]);
        normals.push([n.x, n.y, n.z]);
        positions.push([p0.x, p0.y, p0.z]);
        normals.push([n.x, n.y, n.z]);
        positions.push([p1.x, p1.y, p1.z]);
        normals.push([n.x, n.y, n.z]);
        indices.extend_from_slice(&[base, base + 2, base + 1]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn begin_load_model(mut commands: Commands, assets: Res<AssetServer>, cli: Res<CliModelPath>) {
    let src = cli.0.clone();
    let file_name = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("model.stl");
    let assets_root = Path::new(ASSETS_DIR).join("assets");
    let dst = assets_root.join("imports").join(file_name);
    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::copy(&src, &dst) {
        eprintln!("Failed to copy {} to assets: {}", src.display(), e);
    }
    info!(
        "Loading STL from {} (copied to {})",
        src.display(),
        dst.display()
    );
    let rel_path = dst
        .strip_prefix(&assets_root)
        .unwrap_or(dst.as_path())
        .to_string_lossy()
        .to_string();
    let handle: Handle<StlMesh> = assets.load(rel_path);
    commands.spawn(StlInstance { handle });
}

fn stl_instantiate_ready(
    stl_assets: Res<Assets<StlMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
    mut q: Query<(Entity, &StlInstance), Without<ModelReady>>,
) {
    let center = plate_center();
    for (ent, inst) in q.iter_mut() {
        if let Some(data) = stl_assets.get(&inst.handle) {
            let mesh_handle = meshes.add(data.to_bevy_mesh());
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb_u8(210, 210, 210),
                metallic: 0.02,
                perceptual_roughness: 0.8,
                ..Default::default()
            });
            let (min, max) = compute_bounds(&data.positions);
            let mesh_center = (min + max) * 0.5;
            let base_z = -min.z;
            let translation = Vec3::new(center.x - mesh_center.x, center.y - mesh_center.y, base_z);
            info!(
                "Instantiating STL: vertices={}, faces={} bounds=({:?} to {:?}) translation={:?}",
                data.positions.len(),
                data.indices.len() / 3,
                min,
                max,
                translation
            );
            commands.entity(ent).insert((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material),
                Transform::from_translation(translation),
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                ModelBounds { min, max },
                Selected,
                ModelReady,
            ));
        }
    }
}

fn select_and_start_drag(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    q_cam: Query<(&Camera, &GlobalTransform)>,
    mut q_models: Query<(Entity, &GlobalTransform, &ModelBounds, Option<&Selected>)>,
    mut commands: Commands,
    mut drag: ResMut<DragState>,
    mut transforms: Query<&mut Transform>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let window = match windows.iter().next() {
        Some(w) => w,
        None => return,
    };
    let cursor = match window.cursor_position() {
        Some(p) => p,
        None => return,
    };
    let (camera, cam_tf) = match q_cam.iter().next() {
        Some(v) => v,
        None => return,
    };
    let Ok(ray) = camera.viewport_to_world(cam_tf, cursor) else {
        return;
    };

    let mut best: Option<(Entity, f32)> = None;
    for (ent, gt, bounds, _) in q_models.iter_mut() {
        if let Some(t) = ray_hit_aabb_local(bounds.min, bounds.max, gt, &ray) {
            if t >= 0.0 {
                if let Some((_, best_t)) = best {
                    if t < best_t {
                        best = Some((ent, t));
                    }
                } else {
                    best = Some((ent, t));
                }
            }
        }
    }

    if let Some((hit_ent, _)) = best {
        for (ent, _, _, sel) in q_models.iter_mut() {
            if ent == hit_ent {
                if sel.is_none() {
                    commands.entity(ent).insert(Selected);
                }
            } else if sel.is_some() {
                commands.entity(ent).remove::<Selected>();
            }
        }
        if let Ok(tf) = transforms.get_mut(hit_ent) {
            let start = tf.translation;
            if let Some(hit) = ray_plane_intersection_world(&ray, Vec3::ZERO, Vec3::Z) {
                drag.active = Some(DragData {
                    entity: hit_ent,
                    start_entity_pos: start,
                    start_hit_world: hit,
                });
                info!(
                    "Selected entity {:?}, starting drag at {:?}",
                    hit_ent, start
                );
            }
        }
    } else {
        for (ent, _, _, sel) in q_models.iter_mut() {
            if sel.is_some() {
                commands.entity(ent).remove::<Selected>();
            }
        }
        drag.active = None;
    }
}

fn update_drag(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    q_cam: Query<(&Camera, &GlobalTransform)>,
    drag: Res<DragState>,
    mut q_tf: Query<&mut Transform>,
) {
    if drag.active.is_none() {
        return;
    }
    if !buttons.pressed(MouseButton::Left) {
        return;
    }
    let (camera, cam_tf) = match q_cam.iter().next() {
        Some(v) => v,
        None => return,
    };
    let window = match windows.iter().next() {
        Some(w) => w,
        None => return,
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(cam_tf, cursor) else {
        return;
    };
    let Some(hit) = ray_plane_intersection_world(&ray, Vec3::ZERO, Vec3::Z) else {
        return;
    };

    if let Some(d) = drag.active.clone() {
        if let Ok(mut tf) = q_tf.get_mut(d.entity) {
            let delta = hit - d.start_hit_world;
            let mut new_pos = d.start_entity_pos + delta;
            new_pos.z = d.start_entity_pos.z;
            tf.translation = new_pos;
        }
    }
}

fn end_drag(buttons: Res<ButtonInput<MouseButton>>, mut drag: ResMut<DragState>) {
    if buttons.just_released(MouseButton::Left) {
        drag.active = None;
    }
}

fn camera_orbit_controls(
    time: Res<Time>,
    mut motion_messages: MessageReader<MouseMotion>,
    mut scroll_messages: MessageReader<MouseWheel>,
    buttons: Res<ButtonInput<MouseButton>>,
    drag: Res<DragState>,
    mut q_cam: Query<(&mut Transform, &mut OrbitCamera)>,
) {
    if drag.active.is_some() {
        for ev in scroll_messages.read() {
            if let Some((mut tf, mut orb)) = q_cam.iter_mut().next() {
                let scroll = match ev.unit {
                    bevy::input::mouse::MouseScrollUnit::Line => ev.y * 50.0,
                    bevy::input::mouse::MouseScrollUnit::Pixel => ev.y,
                };
                let scale = (1.0_f32 - scroll * 0.001).clamp(0.1, 10.0);
                orb.distance *= scale;
                orb.distance = orb.distance.clamp(10.0, 10_000.0);
                let offset = offset_from_spherical(orb.distance, orb.yaw, orb.pitch);
                tf.translation = orb.target + offset;
                tf.look_at(orb.target, Vec3::Z);
            }
        }
        return;
    }

    let (mut tf, mut orb) = match q_cam.iter_mut().next() {
        Some(v) => v,
        None => return,
    };

    let mut delta = Vec2::ZERO;
    for ev in motion_messages.read() {
        delta += ev.delta;
    }

    if buttons.pressed(MouseButton::Left) {
        let sens = 0.3f32;
        orb.yaw -= delta.x.to_radians() * sens;
        orb.pitch += delta.y.to_radians() * sens;
        let limit = 89f32.to_radians();
        orb.pitch = orb.pitch.clamp(-limit, limit);
    }

    if buttons.pressed(MouseButton::Right) {
        let pan_speed = orb.distance * 0.0015;
        let forward = (orb.target - tf.translation).normalize_or_zero();
        let right = forward.cross(Vec3::Z).normalize_or_zero();
        let up = Vec3::Z;
        orb.target += (-right * delta.x + up * delta.y) * pan_speed;
    }

    for ev in scroll_messages.read() {
        let scroll = match ev.unit {
            bevy::input::mouse::MouseScrollUnit::Line => ev.y * 50.0,
            bevy::input::mouse::MouseScrollUnit::Pixel => ev.y,
        };
        let scale = (1.0_f32 - scroll * 0.001).clamp(0.1, 10.0);
        orb.distance *= scale;
        orb.distance = orb.distance.clamp(10.0, 10_000.0);
    }

    let offset = offset_from_spherical(orb.distance, orb.yaw, orb.pitch);
    tf.translation = orb.target + offset;
    tf.look_at(orb.target, Vec3::Z);

    let _ = time.delta_secs();
}

fn draw_grid_gizmos(mut gizmos: Gizmos) {
    let squares_per_axis = (GRID_BLOCKS * GRID_BLOCK_SIZE) as i32;
    let extent = plate_extent();
    let minor = Color::srgb_u8(70, 74, 80);
    let major = Color::srgb_u8(110, 114, 120);

    for i in 0..=squares_per_axis {
        let x = i as f32 * GRID_SQUARE_MM;
        let y = i as f32 * GRID_SQUARE_MM;
        let is_major = i % (GRID_BLOCK_SIZE as i32) == 0;
        let col = if is_major { major } else { minor };
        gizmos.line(Vec3::new(x, 0.0, 0.0), Vec3::new(x, extent, 0.0), col);
        gizmos.line(Vec3::new(0.0, y, 0.0), Vec3::new(extent, y, 0.0), col);
    }
}

fn offset_from_spherical(distance: f32, yaw: f32, pitch: f32) -> Vec3 {
    let x = distance * pitch.cos() * yaw.cos();
    let y = distance * pitch.cos() * yaw.sin();
    let z = distance * pitch.sin();
    Vec3::new(x, y, z)
}

fn ray_hit_aabb_local(
    min: Vec3,
    max: Vec3,
    model_gt: &GlobalTransform,
    ray: &bevy::math::Ray3d,
) -> Option<f32> {
    let inv = model_gt.affine().inverse();
    let o = inv.transform_point3(ray.origin);
    let d = inv.transform_vector3((*ray.direction).into());
    let mut tmin = f32::NEG_INFINITY;
    let mut tmax = f32::INFINITY;
    for i in 0..3 {
        let origin_i = o[i];
        let dir_i = d[i];
        let (min_i, max_i) = (min[i], max[i]);
        if dir_i.abs() < 1e-8 {
            if origin_i < min_i || origin_i > max_i {
                return None;
            }
        } else {
            let invd = 1.0 / dir_i;
            let mut t1 = (min_i - origin_i) * invd;
            let mut t2 = (max_i - origin_i) * invd;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }
    Some(tmin)
}

fn ray_plane_intersection_world(
    ray: &bevy::math::Ray3d,
    plane_origin: Vec3,
    plane_normal: Vec3,
) -> Option<Vec3> {
    let ro = ray.origin;
    let rd: Vec3 = (*ray.direction).into();
    let denom = plane_normal.dot(rd);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = plane_normal.dot(plane_origin - ro) / denom;
    if t < 0.0 {
        return None;
    }
    Some(ro + rd * t)
}
