use manifold::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 20mm cube
    let cube = Solid3::cube(20.0);

    // 10mm radius, 30mm tall cylinder sitting on top of the cube
    let pillar = Solid3::cylinder_z(5.0, 30.0, 48).translate(Vec3::new(0.0, 0.0, 15.0));

    let model = cube.merge(&pillar);
    model.write_stl_binary("out.stl")?;
    println!("Wrote out.stl");
    Ok(())
}
