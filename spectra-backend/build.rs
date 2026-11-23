//! Valence model codegen for the saved-dashboard store (`ssr` only).

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=schemas/");
    let target = std::env::var("TARGET").unwrap_or_default();
    if std::env::var_os("CARGO_FEATURE_SSR").is_none() || target.contains("wasm32") {
        return Ok(());
    }
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    valence_codegen::generate_models(&valence_codegen::CodegenConfig {
        schemas_dir: std::path::PathBuf::from("schemas"),
        out_dir,
        file_suffix: "_valence_schema.rs",
        trait_file_suffix: "_valence_trait.rs",
    })?;
    Ok(())
}
