use graphics2d_format::LoadError;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let deck = playing_card_assets::Deck::load(
        &std::fs::read_to_string(root.join("library.json"))?,
        &std::fs::read_to_string(root.join("deck.json"))?,
        |path| std::fs::read_to_string(root.join(path)).map_err(|e| LoadError(e.to_string())),
    )?;
    println!(
        "Validated {} SVG sources, {} styles, {} documents, and all 52 card bindings.",
        deck.context.resources.assets.len(),
        deck.context.resources.styles.len(),
        deck.context.document_count()
    );
    Ok(())
}
