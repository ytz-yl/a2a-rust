#!/usr/bin/env python3
"""
REST Python Client Test for A2A Implementation

This test demonstrates how to use Python to communicate with a REST A2A server.
It shows the REST transport interface and how to send requests over HTTP/REST.
"""

import asyncio
import sys
import uuid
import json
import aiohttp
from typing import AsyncIterator, Dict, Any

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
    print("   Continuing with basic REST test...")

async def test_direct_http_requests():
    """Test basic HTTP requests to REST endpoints"""
    print("🔗 Test 1: Testing direct HTTP requests to REST endpoints...")
    
    base_url = "http://localhost:8081"
    
    async with aiohttp.ClientSession() as session:
        # Test 1.1: Get agent card
        print("\n📋 Test 1.1: GET /agent/card...")
        try:
            async with session.get(f"{base_url}/agent/card") as response:
                status = response.status
                if status == 200:
                    data = await response.json()
                    print(f"✅ Agent card retrieved successfully")
                    print(f"   Status: {status}")
                    print(f"   Agent name: {data.get('name', 'Unknown')}")
                    print(f"   Agent description: {data.get('description', 'Unknown')}")
                    return True
                else:
                    print(f"❌ Failed to get agent card: HTTP {status}")
                    return False
        except Exception as e:
            print(f"❌ HTTP request failed: {e}")
            print("💡 Make sure the REST server is running on port 8081")
            return False

async def test_message_send_via_http():
    """Test sending message via direct HTTP POST"""
    print("\n📤 Test 2: Testing message send via HTTP POST...")
    
    base_url = "http://localhost:8081"
    
    async with aiohttp.ClientSession() as session:
        # Prepare message payload
        message_payload = {
            "message": {
                "kind": "message",
                "messageId": str(uuid.uuid4()),
                "role": "user",
                "parts": [
                    {
                        "kind": "text",
                        "text": "Hello from Python REST client!"
                    },
                    {
                        "kind": "data",
                        "data": {
                            "test": "REST transport",
                            "client": "Python",
                            "timestamp": "2023-10-27T10:00:00Z"
                        }
                    }
                ],
                "contextId": "test-context-python",
                "taskId": "test-task-python"
            }
        }
        
        try:
            async with session.post(
                f"{base_url}/message/send",
                json=message_payload,
                headers={"Content-Type": "application/json"}
            ) as response:
                status = response.status
                print(f"✅ Message send HTTP response: {status}")
                
                if status == 200:
                    data = await response.json()
                    print(f"   Response type: {type(data).__name__}")
                    
                    # Check if response is a task or message
                    if isinstance(data, dict):
                        if "kind" in data:
                            print(f"   Response kind: {data['kind']}")
                            if data["kind"] == "task":
                                print(f"   Task ID: {data.get('id', 'Unknown')}")
                                print(f"   Task state: {data.get('status', {}).get('state', 'Unknown')}")
                            elif data["kind"] == "message":
                                print(f"   Message ID: {data.get('messageId', 'Unknown')}")
                                print(f"   Message role: {data.get('role', 'Unknown')}")
                    
                    return True
                else:
                    error_text = await response.text()
                    print(f"❌ Message send failed: {status}")
                    print(f"   Error: {error_text[:200]}...")
                    return False
                    
        except Exception as e:
            print(f"❌ HTTP request failed: {e}")
            return False

async def test_task_operations_via_http():
    """Test task-related operations via HTTP"""
    print("\n📋 Test 3: Testing task operations via HTTP...")
    
    base_url = "http://localhost:8081"
    task_id = "test-task-123"
    
    async with aiohttp.ClientSession() as session:
        # Test 3.1: Get task
        print(f"\n📋 Test 3.1: GET /tasks/{task_id}...")
        try:
            async with session.get(f"{base_url}/tasks/{task_id}") as response:
                status = response.status
                if status == 200:
                    data = await response.json()
                    print(f"✅ Task retrieved successfully")
                    print(f"   Task ID: {data.get('id', 'Unknown')}")
                    print(f"   Task state: {data.get('status', {}).get('state', 'Unknown')}")
                elif status == 404:
                    print(f"⚠️  Task not found (expected for non-existent task)")
                else:
                    print(f"❌ Failed to get task: HTTP {status}")
        except Exception as e:
            print(f"❌ HTTP request failed: {e}")
        
        # Test 3.2: Cancel task
        print(f"\n📋 Test 3.2: POST /tasks/{task_id}/cancel...")
        try:
            async with session.post(f"{base_url}/tasks/{task_id}/cancel") as response:
                status = response.status
                if status == 200:
                    data = await response.json()
                    print(f"✅ Task cancellation attempted")
                    print(f"   Response status: {status}")
                else:
                    print(f"⚠️  Task cancel returned: HTTP {status}")
        except Exception as e:
            print(f"❌ HTTP request failed: {e}")
    
    return True

async def test_push_notification_operations():
    """Test push notification operations via HTTP"""
    print("\n📋 Test 4: Testing push notification operations via HTTP...")
    
    base_url = "http://localhost:8081"
    task_id = "test-task-123"
    config_id = "test-config"
    
    async with aiohttp.ClientSession() as session:
        # Test 4.1: Set push notification config
        print(f"\n📋 Test 4.1: POST /tasks/{task_id}/push_notifications...")
        
        push_config = {
            "task_id": task_id,
            "push_notification_config": {
                "id": config_id,
                "url": "https://example.com/webhook",
                "token": "test-token-123",
                "authentication": None
            }
        }
        
        try:
            async with session.post(
                f"{base_url}/tasks/{task_id}/push_notifications",
                json=push_config,
                headers={"Content-Type": "application/json"}
            ) as response:
                status = response.status
                if status == 200:
                    data = await response.json()
                    print(f"✅ Push notification config set")
                    print(f"   Config ID: {data.get('push_notification_config', {}).get('id', 'Unknown')}")
                else:
                    print(f"⚠️  Set push config returned: HTTP {status}")
                    if status != 404:  # 404 might be expected if task doesn't exist
                        error_text = await response.text()
                        print(f"   Error: {error_text[:200]}...")
        except Exception as e:
            print(f"❌ HTTP request failed: {e}")
        
        # Test 4.2: Get push notification config
        print(f"\n📋 Test 4.2: GET /tasks/{task_id}/push_notifications...")
        try:
            async with session.get(f"{base_url}/tasks/{task_id}/push_notifications") as response:
                status = response.status
                if status == 200:
                    data = await response.json()
                    print(f"✅ Push notification config retrieved")
                    print(f"   Config ID: {data.get('push_notification_config', {}).get('id', 'Unknown')}")
                else:
                    print(f"⚠️  Get push config returned: HTTP {status}")
        except Exception as e:
            print(f"❌ HTTP request failed: {e}")
    
    return True

async def test_a2a_python_rest_transport():
    """Test a2a-python with REST transport if available"""
    if not HAS_A2A_PYTHON:
        print("\n⚠️  Skipping a2a-python REST test (package not available)")
        return False
    
    print("\n📡 Test 5: Testing a2a-python with REST transport...")
    
    server_url = "http://localhost:8081"
    
    try:
        # Configure client for REST
        config = ClientConfig(
            streaming=True,
            polling=False,
        )
        
        print(f"🔗 Connecting to REST server at {server_url}...")
        
        # Create client using ClientFactory
        client = await ClientFactory.connect(
            agent=server_url,
            client_config=config,
        )
        
        # Get agent card
        agent_card = await client.get_card()
        print(f"✅ Successfully connected via a2a-python REST:")
        print(f"   Agent: {agent_card.name}")
        print(f"   Description: {agent_card.description}")
        
        # Test message sending
        print("\n📤 Testing message send via REST...")
        test_message = Message(
            role=Role.user,
            parts=[
                Part(root=TextPart(text="Hello from Python REST client via a2a-python!"))
            ],
            message_id=str(uuid.uuid4()),
            context_id="test-context-a2a-python"
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
            print("✅ REST message exchange successful via a2a-python")
        else:
            print("⚠️  No events received (server might not be responding)")
            
        return True
        
    except Exception as e:
        print(f"❌ a2a-python REST test failed: {e}")
        return False

async def main():
    """Main test function"""
    print("🚀 A2A Python REST Client Test")
    print("=" * 60)
    
    test_results = []
    
    # Test 1: Basic HTTP requests
    result1 = await test_direct_http_requests()
    test_results.append(("Direct HTTP Requests", result1))
    
    # Test 2: Message send via HTTP
    if result1:  # Only run if server is accessible
        result2 = await test_message_send_via_http()
        test_results.append(("Message Send via HTTP", result2))
    
    # Test 3: Task operations
    if result1:
        result3 = await test_task_operations_via_http()
        test_results.append(("Task Operations", result3))
    
    # Test 4: Push notification operations
    if result1:
        result4 = await test_push_notification_operations()
        test_results.append(("Push Notification Operations", result4))
    
    # Test 5: a2a-python REST transport (if available)
    if HAS_A2A_PYTHON and result1:
        result5 = await test_a2a_python_rest_transport()
        test_results.append(("a2a-python REST Transport", result5))
    
    # Print summary
    print("\n" + "=" * 60)
    print("🎯 REST Test Summary:")
    print("-" * 60)
    
    for test_name, passed in test_results:
        status = "✅ PASS" if passed else "❌ FAIL"
        print(f"{status}: {test_name}")
    
    total_tests = len(test_results)
    passed_tests = sum(1 for _, passed in test_results if passed)
    
    print("-" * 60)
    print(f"📊 Results: {passed_tests}/{total_tests} tests passed")
    
    if passed_tests == total_tests:
        print("🎉 All REST tests passed!")
    else:
        print("⚠️  Some tests failed - check server implementation")
        print("💡 Make sure:")
        print("   - REST server is running on port 8081")
        print("   - Server implements required REST endpoints")
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