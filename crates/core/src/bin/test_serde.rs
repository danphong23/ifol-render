use ifol_render_ecs::scene::MaterialV2;
use ifol_render_ecs::ecs::components::meta::Materials;

fn main() {
    let json = r#"
    [
        {
            "shader_id": "glow",
            "float_uniforms": {
                "u0_r": { "keyframes": [{"time": 0.0, "value": 0.0}] }
            }
        }
    ]
    "#;
    
    match serde_json::from_str::<Materials>(json) {
        Ok(m) => println!("Success: {:#?}", m),
        Err(e) => println!("Error: {}", e),
    }
}
