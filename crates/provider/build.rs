fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("PROTOC").is_none() {
        let protoc = protoc_bin_vendored::protoc_bin_path()?;
        // Build scripts are single-purpose processes; setting PROTOC here only
        // affects prost/tonic code generation in this process and its children.
        unsafe {
            std::env::set_var("PROTOC", protoc);
        }
    }

    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(&["proto/tron/api/api.proto"], &["proto/tron", "proto"])?;
    Ok(())
}
