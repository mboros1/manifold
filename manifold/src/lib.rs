pub mod prelude {
    pub use crate::{Solid3, Triangle};
    pub use glam::{Mat4, Quat, Vec3};
}

use glam::{Mat4, Quat, Vec3};

/// A single triangle in 3D space.
#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
}

/// A watertight triangle mesh representing a solid.
/// For now this is just a bag of triangles; later you can
/// back this with CSG / F-rep / parametric solids.
#[derive(Clone, Debug, Default)]
pub struct Solid3 {
    pub triangles: Vec<Triangle>,
}

impl Solid3 {
    pub fn new(triangles: Vec<Triangle>) -> Self {
        Self { triangles }
    }

    /// Very dumb axis-aligned cube for now, centered at origin.
    /// Size is edge length in whatever units you choose (mm is natural).
    pub fn cube(size: f32) -> Self {
        let h = size * 0.5;

        let p000 = Vec3::new(-h, -h, -h);
        let p001 = Vec3::new(-h, -h,  h);
        let p010 = Vec3::new(-h,  h, -h);
        let p011 = Vec3::new(-h,  h,  h);
        let p100 = Vec3::new( h, -h, -h);
        let p101 = Vec3::new( h, -h,  h);
        let p110 = Vec3::new( h,  h, -h);
        let p111 = Vec3::new( h,  h,  h);

        // 12 triangles (2 per face), right-handed.
        let mut tris = Vec::with_capacity(12);

        // -X face
        tris.push(Triangle { a: p000, b: p001, c: p011 });
        tris.push(Triangle { a: p000, b: p011, c: p010 });

        // +X face
        tris.push(Triangle { a: p100, b: p110, c: p111 });
        tris.push(Triangle { a: p100, b: p111, c: p101 });

        // -Y face
        tris.push(Triangle { a: p000, b: p100, c: p101 });
        tris.push(Triangle { a: p000, b: p101, c: p001 });

        // +Y face
        tris.push(Triangle { a: p010, b: p011, c: p111 });
        tris.push(Triangle { a: p010, b: p111, c: p110 });

        // -Z face
        tris.push(Triangle { a: p000, b: p010, c: p110 });
        tris.push(Triangle { a: p000, b: p110, c: p100 });

        // +Z face
        tris.push(Triangle { a: p001, b: p101, c: p111 });
        tris.push(Triangle { a: p001, b: p111, c: p011 });

        Self::new(tris)
    }

    /// Cylinder aligned to +Z, centered at origin.
    /// `radius` and `height` are in whatever units you choose; `segments` controls roundness.
    pub fn cylinder_z(radius: f32, height: f32, segments: u32) -> Self {
        assert!(segments >= 3, "cylinder_z: need at least 3 segments");
        let h = height * 0.5;

        let mut tris = Vec::with_capacity((segments as usize) * 4);

        let top_center = Vec3::new(0.0, 0.0, h);
        let bot_center = Vec3::new(0.0, 0.0, -h);

        for i in 0..segments {
            let t0 = i as f32 / segments as f32;
            let t1 = (i + 1) as f32 / segments as f32;

            let a = angle_to_xy(radius, t0);
            let b = angle_to_xy(radius, t1);

            let top_a = Vec3::new(a.x, a.y, h);
            let top_b = Vec3::new(b.x, b.y, h);
            let bot_a = Vec3::new(a.x, a.y, -h);
            let bot_b = Vec3::new(b.x, b.y, -h);

            // Top cap (CCW from +Z)
            tris.push(Triangle {
                a: top_center,
                b: top_b,
                c: top_a,
            });

            // Bottom cap (CCW from -Z)
            tris.push(Triangle {
                a: bot_center,
                b: bot_a,
                c: bot_b,
            });

            // Side quad split into two tris, outward facing
            tris.push(Triangle {
                a: bot_a,
                b: bot_b,
                c: top_b,
            });
            tris.push(Triangle {
                a: bot_a,
                b: top_b,
                c: top_a,
            });
        }

        Self::new(tris)
    }

    /// General transform by a 4x4 matrix.
    pub fn transform(mut self, m: Mat4) -> Self {
        for tri in &mut self.triangles {
            tri.a = m.transform_point3(tri.a);
            tri.b = m.transform_point3(tri.b);
            tri.c = m.transform_point3(tri.c);
        }
        self
    }

    /// Translate by an offset.
    pub fn translate(self, offset: Vec3) -> Self {
        self.transform(Mat4::from_translation(offset))
    }

    /// Uniform scale about the origin.
    pub fn scale_uniform(self, s: f32) -> Self {
        self.transform(Mat4::from_scale(Vec3::splat(s)))
    }

    /// Non-uniform scale about the origin.
    pub fn scale_non_uniform(self, s: Vec3) -> Self {
        self.transform(Mat4::from_scale(s))
    }

    /// Rotate about an axis through the origin by radians.
    pub fn rotate_axis_angle(self, axis: Vec3, angle_rad: f32) -> Self {
        let q = Quat::from_axis_angle(axis.normalize_or_zero(), angle_rad);
        self.transform(Mat4::from_quat(q))
    }

    /// Simple mesh concatenation "union".
    pub fn merge(mut self, other: &Solid3) -> Self {
        self.triangles.extend_from_slice(&other.triangles);
        self
    }

    /// Export this solid as a binary STL file.
    pub fn write_stl_binary<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), std::io::Error> {
        use std::fs::File;
        use std::io::BufWriter;

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let stl_triangles: Vec<stl_io::Triangle> = self
            .triangles
            .iter()
            .map(|tri| stl_io::Triangle {
                normal: stl_io::Normal::new([0.0, 0.0, 0.0]), // let downstream recompute normals
                vertices: [
                    stl_io::Vertex::new([tri.a.x, tri.a.y, tri.a.z]),
                    stl_io::Vertex::new([tri.b.x, tri.b.y, tri.b.z]),
                    stl_io::Vertex::new([tri.c.x, tri.c.y, tri.c.z]),
                ],
            })
            .collect();

        stl_io::write_stl(&mut writer, stl_triangles.iter())
    }
}

fn angle_to_xy(radius: f32, t: f32) -> Vec3 {
    let theta = t * std::f32::consts::TAU;
    Vec3::new(radius * theta.cos(), radius * theta.sin(), 0.0)
}
