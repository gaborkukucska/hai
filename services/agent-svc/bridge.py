"""
# START OF FILE /home/tom/hai/services/agent-svc/bridge.py
Python gRPC Sidecar for TrippleEffect core logic.

Wraps TrippleEffect's AgentManager, CycleHandler, and core LLM providers
in a gRPC service that can be called by hainet-persona in Rust.
"""

import sys
import os
import json
import asyncio
import logging
from typing import Dict, Any, Optional

import grpc
from grpc import aio

import agent_pb2
import agent_pb2_grpc

# Ensure TrippleEffect is in path
HAI_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
TE_ROOT = os.path.join(HAI_ROOT, "_workspace", "TrippleEffect")
sys.path.insert(0, TE_ROOT)

from src.agents.manager import AgentManager
from src.agents.core import Agent
from src.agents.constants import (
    AGENT_STATUS_IDLE, AGENT_STATUS_PROCESSING, AGENT_STATUS_ERROR,
    ADMIN_STATE_STARTUP, PM_STATE_STARTUP, WORKER_STATE_STARTUP
)
from src.config.settings import settings
from src.llm_providers.openai_provider import OpenAIProvider
from src.llm_providers.ollama_provider import OllamaProvider
from src.llm_providers.vllm_provider import VllmProvider
from src.llm_providers.openrouter_provider import OpenRouterProvider

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("agent-svc")

class AgentServiceServicer(agent_pb2_grpc.AgentServiceServicer):
    def __init__(self):
        # We initialize a minimal TE AgentManager to hold state
        # In standalone mode we don't use TE's full dependencies via kwargs
        self.manager = AgentManager()
        # Mock UI sending to prevent errors
        self.manager.send_to_ui = self._mock_send_to_ui
        self.manager.push_agent_status_update = self._mock_push_status
        logger.info("AgentService initialized")

    async def _mock_send_to_ui(self, msg: Dict[str, Any]):
        pass
        
    async def _mock_push_status(self, agent_id: str):
        pass

    def _create_provider(self, provider_name: str, config: Dict[str, Any]):
        """Instantiate the correct LLM provider class"""
        name = provider_name.lower()
        if name == "openai":
            return OpenAIProvider(config)
        elif name == "ollama":
            return OllamaProvider(config)
        elif name == "vllm":
            return VllmProvider(config)
        elif name in ("openrouter", "anthropic", "google"):
            # OpenRouter acts as gateway for many providers
            return OpenRouterProvider(config)
        else:
            logger.warning(f"Unknown provider '{provider_name}', falling back to OpenAI")
            return OpenAIProvider(config)

    async def InitializeAgent(self, request, context):
        """Create or restore an agent instance in memory"""
        try:
            agent_id = request.config.agent_id
            
            # Create a mock config dict as expected by TE's Agent class
            te_config = {
                "agent_id": agent_id,
                "config": {
                    "provider": request.config.provider,
                    "model": request.config.model,
                    "temperature": request.config.temperature,
                    "persona": request.config.persona,
                    "role": "Agent",
                    "agent_type": request.config.agent_type,
                    "system_prompt": request.config.system_prompt
                }
            }
            
            provider = self._create_provider(request.config.provider, te_config["config"])
            agent = Agent(te_config, provider, self.manager)
            
            # Set initial state based on type
            if request.config.agent_type == "admin":
                agent.state = ADMIN_STATE_STARTUP
            elif request.config.agent_type == "pm":
                agent.state = PM_STATE_STARTUP
            else:
                agent.state = WORKER_STATE_STARTUP
                
            self.manager.agents[agent_id] = agent
            
            return agent_pb2.InitializeResponse(
                success=True,
                current_state=agent.state,
                error_message=""
            )
        except Exception as e:
            logger.error(f"Error initializing agent: {e}", exc_info=True)
            return agent_pb2.InitializeResponse(
                success=False,
                current_state="",
                error_message=str(e)
            )

    async def SendMessage(self, request, context):
        """Stream response from agent"""
        agent_id = request.agent_id
        agent = self.manager.agents.get(agent_id)
        
        if not agent:
            yield agent_pb2.MessageResponse(
                type=agent_pb2.MessageResponse.ERROR,
                content=f"Agent {agent_id} not found"
            )
            return

        try:
            # 1. Add user message to history
            agent.message_history.append({
                "role": request.role,
                "content": request.content
            })
            
            # 2. Set status to processing
            agent.set_status(AGENT_STATUS_PROCESSING)
            
            # 3. Stream from provider
            from contextlib import aclosing
            
            # Note: We are doing a direct stream_completion here rather than 
            # invoking the full cycle_handler run_cycle, because hainet-persona
            # runs the cycle loop in Rust. We just use Python for the heavy
            # lifting of provider abstraction and token tracking.
            
            async with aclosing(agent.llm_provider.stream_completion(
                messages=agent.message_history,
                model=agent.model,
                temperature=agent.temperature,
                # Max tokens could be drawn from config later
            )) as stream:
                
                full_content = ""
                async for event in stream:
                    event_type = event.get("type")
                    
                    if event_type == "response_chunk":
                        chunk = event.get("content", "")
                        full_content += chunk
                        yield agent_pb2.MessageResponse(
                            type=agent_pb2.MessageResponse.TEXT_CHUNK,
                            content=chunk
                        )
                    
                    elif event_type == "tool_call_start":
                        tc_id = event.get("tool_call_id", "")
                        name = event.get("tool_name", "")
                        yield agent_pb2.MessageResponse(
                            type=agent_pb2.MessageResponse.TOOL_CALL_START,
                            content=name,
                            tool_call_id=tc_id
                        )
                        
                    elif event_type == "tool_call_chunk":
                        tc_id = event.get("tool_call_id", "")
                        args_chunk = event.get("arguments_chunk", "")
                        yield agent_pb2.MessageResponse(
                            type=agent_pb2.MessageResponse.TOOL_CALL_CHUNK,
                            content=args_chunk,
                            tool_call_id=tc_id
                        )
                        
                    elif event_type == "tool_call_end":
                        tc_id = event.get("tool_call_id", "")
                        yield agent_pb2.MessageResponse(
                            type=agent_pb2.MessageResponse.TOOL_CALL_END,
                            tool_call_id=tc_id
                        )
                        
                    elif event_type == "error":
                        yield agent_pb2.MessageResponse(
                            type=agent_pb2.MessageResponse.ERROR,
                            content=event.get("content", "Unknown error")
                        )
            
            # Let Rust handle adding the assistant message and parsing XML
            yield agent_pb2.MessageResponse(
                type=agent_pb2.MessageResponse.COMPLETE
            )
            
        except Exception as e:
            logger.error(f"Error streaming message: {e}", exc_info=True)
            yield agent_pb2.MessageResponse(
                type=agent_pb2.MessageResponse.ERROR,
                content=str(e)
            )
        finally:
            agent.set_status(AGENT_STATUS_IDLE)

    async def GetAgentState(self, request, context):
        agent_id = request.agent_id
        agent = self.manager.agents.get(agent_id)
        
        if not agent:
            return agent_pb2.StateResponse(current_state="NOT_FOUND", current_status="error", cycles_without_transition=0)
            
        return agent_pb2.StateResponse(
            current_state=agent.state or "unknown",
            current_status=agent.status or "idle",
            cycles_without_transition=getattr(agent, '_cycles_without_transition', 0)
        )

    async def TransitionState(self, request, context):
        agent_id = request.agent_id
        agent = self.manager.agents.get(agent_id)
        
        if not agent:
            return agent_pb2.TransitionResponse(success=False, error_message=f"Agent {agent_id} not found")
            
        old_state = agent.state
        agent.state = request.new_state
        logger.info(f"Agent {agent_id} transitioned from {old_state} to {request.new_state} (Reason: {request.reason})")
        
        return agent_pb2.TransitionResponse(success=True, error_message="")

    async def TerminateAgent(self, request, context):
        agent_id = request.agent_id
        if agent_id in self.manager.agents:
            del self.manager.agents[agent_id]
            logger.info(f"Agent {agent_id} terminated")
            return agent_pb2.TerminateResponse(success=True)
        return agent_pb2.TerminateResponse(success=False)

async def serve():
    # Initialize the database to prevent db_manager errors
    from src.core.database_manager import db_manager
    try:
        await db_manager._initialize_db()
        logger.info("TrippleEffect database initialized for bridge.")
    except Exception as e:
        logger.error(f"Failed to initialize TrippleEffect database: {e}", exc_info=True)

    port = os.getenv("AGENT_SVC_PORT", "50051")
    server = aio.server()
    agent_pb2_grpc.add_AgentServiceServicer_to_server(AgentServiceServicer(), server)
    server.add_insecure_port(f'[::]:{port}')
    
    logger.info(f"Starting agent-svc on port {port}")
    await server.start()
    await server.wait_for_termination()

if __name__ == '__main__':
    asyncio.run(serve())
