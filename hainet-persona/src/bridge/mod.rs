use serde::{Deserialize, Serialize};

pub mod agent_pb {
    tonic::include_proto!("agent");
}

pub mod client;
pub mod sidecar;
