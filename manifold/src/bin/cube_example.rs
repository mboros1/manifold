use manifold::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = Solid3::extrude(&Profile2::rectangle(20.0, 20.0), 20.0);

    // Lathed knob profile: radius vs z, spun around the Z axis
    let knob_profile = vec![
        Vec2::new(0.0, -15.0),
        Vec2::new(6.0, -15.0),
        Vec2::new(8.0, 0.0),
        Vec2::new(5.0, 12.0),
        Vec2::new(0.0, 15.0),
    ];
    let knob = Solid3::lathe_z(&knob_profile, 64).translate(Vec3::new(0.0, 0.0, 25.0));

    let model = base.merge(&knob);
    model.write_stl_binary("out.stl")?;
    println!("Wrote out.stl");
    Ok(())
}
