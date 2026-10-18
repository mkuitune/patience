use graphics2d_format::{Context, DeckBindings, LoadError, proto};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    println!("cargo:rerun-if-changed=library.json");
    println!("cargo:rerun-if-changed=deck.json");
    let manifest = std::fs::read_to_string(root.join("library.json"))?;
    let context = Context::load(&manifest, |path| {
        println!("cargo:rerun-if-changed={path}");
        std::fs::read_to_string(root.join(path)).map_err(|e| LoadError(e.to_string()))
    })?;
    DeckBindings::load(&std::fs::read_to_string(root.join("deck.json"))?, &context)?;
    let library: proto::Library = serde_json::from_str(&manifest)?;
    let mut source = String::from("pub const BUNDLED: &[(&str, &str)] = &[\n");
    for resource in library.resources {
        source.push_str(&format!(
            "({:?}, include_str!({:?})),\n",
            resource.path,
            root.join(&resource.path)
        ));
    }
    source.push_str("];\n");
    std::fs::write(
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("bundled.rs"),
        source,
    )?;
    Ok(())
}
