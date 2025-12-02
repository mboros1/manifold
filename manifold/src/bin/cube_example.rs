use manifold::prelude::*;

fn main() {
    // 20mm cube
    let cube = Solid3::cube(20.0);
    cube.write_stl_binary("out.stl").expect("failed to write STL");
    println!("Wrote out.stl");
}
