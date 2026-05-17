import grpc
import agent_pb2
import agent_pb2_grpc

def test():
    channel = grpc.insecure_channel('localhost:50051')
    stub = agent_pb2_grpc.AgentServiceStub(channel)
    
    # Try to initialize an agent to test the bridge connection
    config = agent_pb2.AgentConfig(
        agent_id="test_worker_1",
        provider="local",
        model="test",
        temperature=0.7,
        persona="You are a helpful test agent.",
        agent_type="worker",
        system_prompt="You are a helpful test agent."
    )
    request = agent_pb2.InitializeRequest(config=config)
    
    try:
        response = stub.InitializeAgent(request)
        print(f"Success: {response.success}")
        print(f"State: {response.current_state}")
        if not response.success:
            print(f"Error: {response.error_message}")
    except Exception as e:
        print(f"RPC failed: {e}")

if __name__ == '__main__':
    test()
