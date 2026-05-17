# Rust-Python Bridge

This module integrates the existing Python gRPC sidecar `services/agent-svc/bridge.py` 
with the Rust core of HAI-Net. It contains the auto-generated Tonic client
as well as a process supervisor to manage the sidecar.

Run `cargo test` from the `hainet-persona` folder to build.
