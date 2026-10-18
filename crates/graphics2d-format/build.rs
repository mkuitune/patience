fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/graphics.proto");
    println!("cargo:rerun-if-env-changed=PROTOC");
    let output = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    let descriptor = output.join("graphics.bin");
    let compiler = match std::env::var_os("PROTOC") {
        Some(path) => std::path::PathBuf::from(path),
        None => protoc_bin_vendored::protoc_bin_path()?,
    };
    prost_build::Config::new()
        .protoc_executable(compiler)
        .file_descriptor_set_path(&descriptor)
        .compile_protos(&["proto/graphics.proto"], &["proto"])?;
    pbjson_build::Builder::new()
        .register_descriptors(&std::fs::read(descriptor)?)?
        .build(&[".graphics.v1"])?;
    Ok(())
}
