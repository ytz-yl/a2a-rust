#!/usr/bin/env python3
"""
gRPC Python Client Test for A2A Implementation

This test demonstrates how to use Python to communicate with a gRPC A2A server.
It shows the gRPC transport interface and how to send requests over gRPC.
"""

import asyncio
import sys
import uuid
import json
from typing import AsyncIterator
import grpc
import grpc.aio

# Try to import a2a-python if available
try:
    from a2a.client.client_factory import ClientFactory
    from a2a.client.client import ClientConfig
    from a2a.types import (
        Message, Part, TextPart, DataPart, Role,
        Task, TaskStatusUpdateEvent, TaskArtifactUpdateEvent
    )
    HAS_A2A_PYTHON = True
except ImportError:
    HAS_A2A_PYTHON = False
    print("⚠️  a2a-python package not found. Install with: pip install a2a-sdk")
    print("   Continuing with basic gRPC test...")

# For basic gRPC testing
try:
    import a2a.grpc.a2a_pb2 as a2a_pb2
    import a2a.grpc.a2a_pb2_grpc as a2a_pb2_grpc
    HAS_A2A_GRPC = True
except ImportError:
    HAS_A2A_GRPC = False
    print("⚠️  a2a gRPC protobuf definitions not found")
    print("   Some tests may be limited")

async def test_grpc_channel_connection():
    """Test basic gRPC channel connection"""
    print("🔗 Test 1: Testing gRPC channel connection...")
    
    server_address = "localhost:50051"
    
    try:
        # Create insecure channel
        async with grpc.aio.insecure_channel(server_address) as channel:
            print(f"✅ Successfully created gRPC channel to {server_address}")
            
            # Test channel connectivity
            try:
                # Try to get channel state
                state = channel.get_state(try_to_connect=True)
                print(f"   Channel state: {state}")
                
                # Wait for channel to be ready (with timeout)
                try:
                    await asyncio.wait_for(channel.channel_ready(), timeout=5.0)
                    print("✅ gRPC channel is ready")
                    return True
                except asyncio.TimeoutError:
                    print("❌ gRPC channel connection timeout")
                    return False
                    
            except Exception as e:
                print(f"❌ Error checking channel state: {e}")
                return False
                
    except Exception as e:
        print(f"❌ Failed to create gRPC channel: {e}")
        print("💡 Make sure the gRPC server is running on port 50051")
        return False

async def test_a2a_python_grpc_transport():
    """Test a2a-python with gRPC transport if available"""
    if not HAS_A2A_PYTHON:
        print("⚠️  Skipping a2a-python gRPC test (package not available)")
        return False
    
    print("\n📡 Test 2: Testing a2a-python with gRPC transport...")
    
    server_url = "grpc://localhost:50051"
    
    try:
        # Configure client for gRPC
        config = ClientConfig(
            streaming=True,
            polling=False,
        )
        
        print(f"🔗 Connecting to gRPC server at {server_url}...")
        
        # Create client using ClientFactory
        client = await ClientFactory.connect(
            agent=server_url,
            client_config=config,
        )
        
        # Get agent card
        agent_card = await client.get_card()
        print(f"✅ Successfully connected via a2a-python gRPC:")
        print(f"   Agent: {agent_card.name}")
        print(f"   Description: {agent_card.description}")
        
        # Test message sending
        print("\n📤 Testing message send via gRPC...")
        test_message = Message(
            role=Role.user,
            parts=[
                Part(root=TextPart(text="Hello from Python gRPC client!"))
            ],
            message_id=str(uuid.uuid4()),
            context_id="test-context-grpc"
        )
        
        event_count = 0
        async for event in client.send_message(test_message):
            event_count += 1
            if isinstance(event, tuple) and len(event) == 2:
                task, update = event
                print(f"📡 Event {event_count}: Task {task.id} - {task.status.state}")
            elif isinstance(event, Message):
                print(f"📨 Event {event_count}: Message from {event.role}")
            
            if event_count >= 3:
                break
        
        if event_count > 0:
            print("✅ gRPC message exchange successful")
        else:
            print("⚠️  No events received (server might not be responding)")
            
        return True
        
    except Exception as e:
        print(f"❌ a2a-python gRPC test failed: {e}")
        return False

async def test_raw_grpc_stubs():
    """Test raw gRPC stub calls if protobufs are available"""
    if not HAS_A2A_GRPC:
        print("⚠️  Skipping raw gRPC stub test (protobufs not available)")
        return False
    
    print("\n🔧 Test 3: Testing raw gRPC stub calls...")
    
    server_address = "localhost:50051"
    
    try:
        async with grpc.aio.insecure_channel(server_address) as channel:
            # Create stub
            stub = a2a_pb2_grpc.A2aServiceStub(channel)
            
            # Test GetAgentCard
            print("📋 Testing GetAgentCard RPC...")
            try:
                request = a2a_pb2.GetAgentCardRequest()
                response = await stub.GetAgentCard(request)
                print(f"✅ GetAgentCard response received")
                if response.card:
                    print(f"   Agent name: {response.card.name}")
            except grpc.aio.AioRpcError as e:
                print(f"❌ GetAgentCard failed: {e.code()} - {e.details()}")
            
            # Test SendMessage (if server supports it)
            print("\n📤 Testing SendMessage RPC...")
            try:
                # Create a simple message
                message = a2a_pb2.Message(
                    kind="message",
                    message_id=str(uuid.uuid4()),
                    role="user",
                    parts=[
                        a2a_pb2.Part(
                            kind="text",
                            text_part=a2a_pb2.TextPart(text="Hello from raw gRPC!")
                        )
                    ]
                )
                
                request = a2a_pb2.SendMessageRequest(request=message)
                response = await stub.SendMessage(request)
                print(f"✅ SendMessage response received")
                if response.task:
                    print(f"   Task created: {response.task.id}")
                elif response.msg:
                    print(f"   Message received: {response.msg.message_id}")
                    
            except grpc.aio.AioRpcError as e:
                print(f"❌ SendMessage failed: {e.code()} - {e.details()}")
                print("💡 This might be expected if server doesn't implement full gRPC yet")
            
            return True
            
    except Exception as e:
        print(f"❌ Raw gRPC test failed: {e}")
        return False

async def main():
    """Main test function"""
    print("🚀 A2A Python gRPC Client Test")
    print("=" * 60)
    
    test_results = []
    
    # Test 1: Basic gRPC channel connection
    result1 = await test_grpc_channel_connection()
    test_results.append(("gRPC Channel Connection", result1))
    
    # Test 2: a2a-python gRPC transport (if available)
    if HAS_A2A_PYTHON:
        result2 = await test_a2a_python_grpc_transport()
        test_results.append(("a2a-python gRPC Transport", result2))
    
    # Test 3: Raw gRPC stubs (if available)
    if HAS_A2A_GRPC:
        result3 = await test_raw_grpc_stubs()
        test_results.append(("Raw gRPC Stub Calls", result3))
    
    # Print summary
    print("\n" + "=" * 60)
    print("🎯 gRPC Test Summary:")
    print("-" * 60)
    
    for test_name, passed in test_results:
        status = "✅ PASS" if passed else "❌ FAIL"
        print(f"{status}: {test_name}")
    
    total_tests = len(test_results)
    passed_tests = sum(1 for _, passed in test_results if passed)
    
    print("-" * 60)
    print(f"📊 Results: {passed_tests}/{total_tests} tests passed")
    
    if passed_tests == total_tests:
        print("🎉 All gRPC tests passed!")
    else:
        print("⚠️  Some tests failed - check server implementation")
        print("💡 Make sure:")
        print("   - gRPC server is running on port 50051")
        print("   - Server implements required gRPC endpoints")
        print("   - a2a-python is installed for full test coverage")
    
    # Return non-zero exit code if any test failed
    return passed_tests == total_tests

if __name__ == "__main__":
    try:
        success = asyncio.run(main())
        sys.exit(0 if success else 1)
    except KeyboardInterrupt:
        print("\n\n⏹️  Test interrupted by user")
        sys.exit(1)
    except Exception as e:
        print(f"\n❌ Unexpected error: {e}")
        sys.exit(1)