fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(false)
        // Keep high-volume binary payloads reference-counted across the tonic
        // decode/encode boundary. Fixed-size identifiers and UTF-8-like fields
        // remain `Vec<u8>` so they do not retain an entire response frame.
        .bytes(".protocol.TransactionExtention.constant_result")
        .bytes(".protocol.TriggerSmartContract.data")
        .bytes(".protocol.SmartContract.bytecode")
        .bytes(".protocol.SmartContractDataWrapper.runtimecode")
        .bytes(".protocol.TransactionInfo.Log.data")
        .bytes(".protocol.Transaction.raw.data")
        .bytes(".protocol.Transaction.signature")
        .compile_protos(&["proto/tron/api/api.proto"], &["proto/tron", "proto"])?;
    Ok(())
}
