use std::sync::Arc;
use tonic::transport::Channel;
use anyhow::{Result, Context, anyhow};
use tracing::{info, warn, error};

use crate::bridge::agent_pb::{
    agent_service_client::AgentServiceClient,
    AgentConfig, InitializeRequest, MessageRequest, MessageResponse,
    StateRequest, TransitionRequest, TerminateRequest,
};

/// Client for communicating with the Python TrippleEffect bridge via gRPC
#[derive(Clone)]
pub struct BridgeClient {
    client: AgentServiceClient<Channel>,
}

impl BridgeClient {
    /// Connect to the bridge service at the given URI
    pub async fn connect(uri: String) -> Result<Self> {
        info!("Connecting to Python agent bridge at {}", uri);
        let client = AgentServiceClient::connect(uri).await
            .context("Failed to connect to agent-svc gRPC server")?;
            
        Ok(Self { client })
    }

    /// Initialize a new agent or restore an existing one
    pub async fn initialize_agent(
        &mut self,
        agent_id: String,
        agent_type: String,
        persona: String,
        provider: String,
        model: String,
        temperature: f32,
        system_prompt: String,
        resume_existing: bool,
    ) -> Result<String> {
        let request = tonic::Request::new(InitializeRequest {
            config: Some(AgentConfig {
                agent_id,
                agent_type,
                persona,
                provider,
                model,
                temperature,
                system_prompt,
            }),
            resume_existing,
        });

        let response = self.client.initialize_agent(request).await?.into_inner();
        
        if response.success {
            Ok(response.current_state)
        } else {
            Err(anyhow!("Failed to initialize agent: {}", response.error_message))
        }
    }

    /// Send a message to the agent and return the response stream
    pub async fn send_message(
        &mut self,
        agent_id: String,
        role: String,
        content: String,
    ) -> Result<tonic::Streaming<MessageResponse>> {
        let request = tonic::Request::new(MessageRequest {
            agent_id,
            role,
            content,
        });

        let response = self.client.send_message(request).await?.into_inner();
        Ok(response)
    }

    /// Get the current state of an agent
    pub async fn get_agent_state(&mut self, agent_id: String) -> Result<(String, String)> {
        let request = tonic::Request::new(StateRequest { agent_id });
        let response = self.client.get_agent_state(request).await?.into_inner();
        
        Ok((response.current_state, response.current_status))
    }

    /// Force a state transition for an agent
    pub async fn transition_state(
        &mut self,
        agent_id: String,
        new_state: String,
        reason: String,
    ) -> Result<()> {
        let request = tonic::Request::new(TransitionRequest {
            agent_id,
            new_state,
            reason,
        });

        let response = self.client.transition_state(request).await?.into_inner();
        
        if response.success {
            Ok(())
        } else {
            Err(anyhow!("Failed to transition state: {}", response.error_message))
        }
    }

    /// Terminate an agent instance
    pub async fn terminate_agent(&mut self, agent_id: String) -> Result<()> {
        let request = tonic::Request::new(TerminateRequest { agent_id });
        let response = self.client.terminate_agent(request).await?.into_inner();
        
        if response.success {
            Ok(())
        } else {
            Err(anyhow!("Failed to terminate agent"))
        }
    }
}
