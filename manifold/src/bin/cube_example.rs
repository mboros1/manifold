use manifold::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = Solid3::extrude(&Profile2::rectangle(20.0, 20.0), 20.0);

    // 10mm diameter post, 30mm tall, centered on top of the base
    let post = Solid3::extrude(&Profile2::circle(5.0, 48), 30.0)
        .translate(Vec3::new(0.0, 0.0, 25.0));

    let model = base.merge(&post);
    model.write_stl_binary("out.stl")?;
    println!("Wrote out.stl");
    Ok(())
}
