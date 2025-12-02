pub mod prelude {
    pub use crate::{Solid3, Triangle};
    pub use glam::Vec3;
}

use glam::Vec3;

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
