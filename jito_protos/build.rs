use tonic_build::configure;

fn main() {
    // protoc must be installed system-wide (e.g. `winget install Google.Protobuf`)
    configure()
        .compile_protos(
            &[
                "protos/auth.proto",
                "protos/shared.proto",
                "protos/shredstream.proto",
            ],
            &["protos"],
        )
        .unwrap();
}

