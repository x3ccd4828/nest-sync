fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This application only consumes the API; server stubs are unnecessary.
    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(&["api.proto"], &["."])?;
    Ok(())
}
