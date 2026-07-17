fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generate only the server (the production crate already has the client).
    // Imports resolve against the real TRON proto tree so the reused domain
    // messages keep their exact field numbers.
    tonic_prost_build::configure().build_client(false).build_server(true).compile_protos(
        &["proto/wallet_solidity_test.proto"],
        &["proto", "../../proto/tron", "../../proto"],
    )?;
    Ok(())
}
