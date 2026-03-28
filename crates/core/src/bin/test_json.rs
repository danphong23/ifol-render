fn main() {
    let s = r#"{"content": "HELLO", "color": [0.38, 0.65, 0.98, 1.0]}"#;
    let t: ifol_render_ecs::ecs::components::TextSource = serde_json::from_str(s).unwrap();
    println!("{:?}", t);
}
