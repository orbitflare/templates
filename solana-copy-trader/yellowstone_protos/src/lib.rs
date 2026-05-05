#![allow(clippy::large_enum_variant)]

pub mod solana_storage {
    tonic::include_proto!("solana.storage.confirmed_block");
}

pub mod geyser {
    tonic::include_proto!("geyser");
}
