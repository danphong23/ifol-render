fn main() {
    let bytes = std::fs::read("crates/wasm/pkg/ifol_render_wasm_bg.wasm").unwrap();
    let s = String::from_utf8_lossy(&bytes);
    if let Some(pos) = s.find("expected one of ") {
        println!("{}", &s[pos..pos+100]);
    } else {
        println!("String not found in WASM");
    }
}
