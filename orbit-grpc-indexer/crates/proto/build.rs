fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = "../../protos";
    let includes = &[proto_dir.to_string()];

    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(&[format!("{proto_dir}/jetstream.proto")], includes)?;

    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(&[format!("{proto_dir}/solana-storage.proto")], includes)?;

    tonic_prost_build::configure()
        .build_server(false)
        .extern_path(
            ".solana.storage.ConfirmedBlock",
            "crate::solana_storage",
        )
        .compile_protos(&[format!("{proto_dir}/geyser.proto")], includes)?;

    Ok(())
}
