# A2A Python ↔ Rust 互操作性测试指南

本指南详细介绍如何测试Python和Rust端之间的互相通信，包括gRPC和REST两种传输协议。

## 📋 测试矩阵

| 测试场景 | 服务器 | 客户端 | 传输协议 | 测试状态 |
|----------|--------|--------|----------|----------|
| 1 | Rust服务器 | Python客户端 | gRPC | ✅ 已验证 |
| 2 | Rust服务器 | Python客户端 | REST | ✅ 已验证 |
| 3 | Python服务器 | Rust客户端 | gRPC | ✅ 测试就绪 |
| 4 | Python服务器 | Rust客户端 | REST | ✅ 测试就绪 |
| 5 | Rust服务器 | Rust客户端 | gRPC | ✅ 已验证 |
| 6 | Rust服务器 | Rust客户端 | REST | ✅ 已验证 |

## 🛠️ 前置条件

### 1. Rust环境
```bash
# 确保在项目根目录
cd c:\Users\mazex\Desktop\a2a-rust

# 检查Cargo工作正常
cargo --version
```

### 2. Python环境
```bash
# 检查Python版本
python --version  # 需要Python 3.8+

# 安装a2a-python SDK（如果需要）
pip install a2a-sdk
```

### 3. 验证测试文件
```bash
# 检查所有测试文件是否存在
ls examples/grpc/
ls examples/rest/
```

## 🚀 测试步骤

### 场景1：Rust服务器 ↔ Python客户端（gRPC）

#### 步骤1：启动Rust gRPC服务器
```bash
# 在终端1中
cd c:\Users\mazex\Desktop\a2a-rust
cargo run --example grpc_rust_server_test
```
预期输出：
```
🚀 Starting gRPC Test Server on 127.0.0.1:50051
✅ gRPC Test Server is ready to accept connections!
📡 Endpoints:
   - gRPC endpoint: grpc://127.0.0.1:50051
   - Agent card: Available via GetAgentCard RPC
```

#### 步骤2：运行Python gRPC客户端
```bash
# 在终端2中
cd c:\Users\mazex\Desktop\a2a-rust\examples\grpc
python python_client_test.py
```
预期输出：
```
🚀 A2A Python gRPC Client Test
==============================================================
🔗 Test 1: Testing gRPC channel connection...
✅ Successfully created gRPC channel to localhost:50051
✅ gRPC channel is ready
✅ All tests passed!
```

#### 步骤3：验证通信
观察两个终端的输出，确保：
- Python客户端成功连接到Rust服务器
- 可以获取agent card信息
- 可以发送消息并接收响应

### 场景2：Rust服务器 ↔ Python客户端（REST）

#### 步骤1：启动Rust REST服务器
```bash
# 在终端1中
cd c:\Users\mazex\Desktop\a2a-rust
cargo run --example rest_rust_server_test
```
预期输出：
```
🚀 Starting REST Test Server on 127.0.0.1:8081
✅ REST Test Server is ready to accept connections!
```

#### 步骤2：运行Python REST客户端
```bash
# 在终端2中
cd c:\Users\mazex\Desktop\a2a-rust\examples\rest
python python_client_test.py
```
预期输出：
```
🚀 A2A Python REST Client Test
==============================================================
🔗 Test 1: Testing direct HTTP requests to REST endpoints...
✅ Agent card retrieved successfully
✅ All tests passed!
```

### 场景3：Python服务器 ↔ Rust客户端（gRPC）

#### 步骤1：启动Python gRPC服务器
```bash
# 在终端1中
cd c:\Users\mazex\Desktop\a2a-rust\examples\grpc
python python_server_test.py --server
```
预期输出：
```
🚀 Starting Python gRPC A2A server...
📡 Python gRPC server starting on 127.0.0.1:50052
✅ Python gRPC server ready (simulated)
```

#### 步骤2：运行Rust gRPC客户端
```bash
# 在终端2中
cd c:\Users\mazex\Desktop\a2a-rust
cargo run --example grpc_rust_client_test -- --server-url grpc://127.0.0.1:50052
```
预期输出：
```
🚀 A2A gRPC Client Test
============================================================
🔗 Attempting to connect to gRPC server at grpc://127.0.0.1:50052...
✅ Successfully connected to gRPC server
✅ Successfully created client via ClientFactory
```

### 场景4：Python服务器 ↔ Rust客户端（REST）

#### 步骤1：启动Python REST服务器
```bash
# 在终端1中
cd c:\Users\mazex\Desktop\a2a-rust\examples\rest
python python_server_test.py --server
```
预期输出：
```
🚀 Starting Python REST A2A server...
📡 Python REST server starting on 127.0.0.1:8082
✅ Python REST server ready (simulated)
```

#### 步骤2：运行Rust REST客户端
```bash
# 在终端2中
cd c:\Users\mazex\Desktop\a2a-rust
cargo run --example rest_rust_client_test -- --server-url http://127.0.0.1:8082
```
预期输出：
```
🚀 A2A REST Client Test
============================================================
🔗 Attempting to connect to REST server at http://127.0.0.1:8082...
✅ Successfully connected to REST server
✅ Successfully created client via ClientFactory
```

## 🔧 测试脚本

为了方便测试，创建以下脚本：

### `test_all_interop.sh`（Linux/macOS）
```bash
#!/bin/bash

echo "🚀 Running all interoperability tests..."

# 测试1: Rust服务器 ↔ Python客户端 (gRPC)
echo "🔧 Test 1: Rust gRPC server ↔ Python client"
cargo run --example grpc_rust_server_test &
RUST_SERVER_PID=$!
sleep 2
cd examples/grpc && python python_client_test.py
kill $RUST_SERVER_PID

echo "✅ Test 1 completed"
echo "---"

# 测试2: Rust服务器 ↔ Python客户端 (REST)
echo "🔧 Test 2: Rust REST server ↔ Python client"
cargo run --example rest_rust_server_test &
REST_SERVER_PID=$!
sleep 2
cd ../rest && python python_client_test.py
kill $REST_SERVER_PID

echo "✅ All interoperability tests completed!"
```

### `test_all_interop.bat`（Windows）
```bat
@echo off
echo 🚀 Running all interoperability tests...

echo 🔧 Test 1: Rust gRPC server ^<-> Python client
start /B cargo run --example grpc_rust_server_test
timeout /t 3
cd examples\grpc
python python_client_test.py
taskkill /F /IM grpc_rust_server_test.exe 2>nul

echo ✅ Test 1 completed
echo ---

echo 🔧 Test 2: Rust REST server ^<-> Python client
start /B cargo run --example rest_rust_server_test
timeout /t 3
cd ..\rest
python python_client_test.py
taskkill /F /IM rest_rust_server_test.exe 2>nul

echo ✅ All interoperability tests completed!
```

## 🧪 手动测试命令

### 基本连接测试

#### gRPC连接测试
```bash
# 使用grpcurl测试gRPC服务器
grpcurl -plaintext localhost:50051 a2a.A2aService/GetAgentCard

# 使用Python测试
python -c "
import grpc
channel = grpc.insecure_channel('localhost:50051')
try:
    grpc.channel_ready_future(channel).result(timeout=5)
    print('✅ gRPC server is accessible')
except Exception as e:
    print(f'❌ gRPC server not accessible: {e}')
"
```

#### REST连接测试
```bash
# 使用curl测试REST服务器
curl http://localhost:8081/agent/card
curl http://localhost:8081/.well-known/agent.json

# 发送测试消息
curl -X POST http://localhost:8081/message/send \
  -H "Content-Type: application/json" \
  -d '{
    "message": {
      "kind": "message",
      "messageId": "test-123",
      "role": "user",
      "parts": [{"kind": "text", "text": "Hello from curl"}]
    }
  }'
```

## 📊 验证点

### 成功标准
1. **连接建立**：客户端成功连接到服务器
2. **Agent Card获取**：能够获取服务器的能力信息
3. **消息交换**：能够发送消息并接收响应
4. **错误处理**：适当的错误处理和恢复

### 验证方法
```bash
# 1. 检查服务器日志
# 应该看到客户端连接和请求处理日志

# 2. 检查客户端输出
# 应该看到成功连接和消息交换

# 3. 检查网络连接
netstat -an | findstr "50051"  # Windows
netstat -an | grep 50051       # Linux/macOS

# 4. 验证数据格式
# 使用JSON格式化工具验证消息格式
```

## 🔍 故障排除

### 常见问题

#### 1. 端口冲突
```
Error: Address already in use (os error 98)
```
**解决方案**：
```bash
# 查找占用端口的进程
netstat -ano | findstr :50051
# 终止进程或更改服务器端口
```

#### 2. Python包缺失
```
ModuleNotFoundError: No module named 'a2a'
```
**解决方案**：
```bash
pip install a2a-sdk
# 或使用虚拟环境中的包
```

#### 3. 连接超时
```
ConnectError: connection refused
```
**解决方案**：
1. 确认服务器正在运行
2. 检查防火墙设置
3. 验证服务器监听地址（127.0.0.1 vs 0.0.0.0）

#### 4. 版本不兼容
```
Error: Invalid protocol version
```
**解决方案**：
1. 确保Python和Rust使用相同的A2A协议版本
2. 检查类型定义对齐（参考docs/python_rust_type_alignment_summary.md）

### 调试技巧

#### 启用详细日志
```bash
# Rust服务器
RUST_LOG=debug cargo run --example grpc_rust_server_test

# Python客户端
python -c "import logging; logging.basicConfig(level=logging.DEBUG)" python_client_test.py
```

#### 网络调试
```bash
# 检查端口监听
lsof -i :50051  # Linux/macOS
netstat -ano | findstr :50051  # Windows

# 测试连接
telnet localhost 50051  # 测试TCP连接
curl -v http://localhost:8081/agent/card  # 测试HTTP连接
```

#### 协议调试
```bash
# 使用Wireshark或tcpdump捕获网络流量
# 分析gRPC/HTTP协议交互

# 使用grpcurl调试gRPC
grpcurl -plaintext -v localhost:50051 list
grpcurl -plaintext -v localhost:50051 describe a2a.A2aService
```

## 📈 性能测试

### 基准测试
```bash
# 使用ab进行HTTP性能测试
ab -n 1000 -c 10 http://localhost:8081/agent/card

# 使用ghz进行gRPC性能测试
ghz --insecure --proto=a2a.proto --call=a2a.A2aService/GetAgentCard localhost:50051
```

### 负载测试
```bash
# 并发消息发送测试
for i in {1..100}; do
  python examples/grpc/python_client_test.py &
done
wait
```

## 📚 高级测试

### 1. 长时间运行测试
```bash
# 运行服务器24小时，定期发送请求
./run_stability_test.sh
```

### 2. 故障恢复测试
```bash
# 模拟网络中断
# 1. 启动服务器和客户端
# 2. 中断网络连接
# 3. 恢复网络连接
# 4. 验证自动重连
```

### 3. 压力测试
```bash
# 发送大量并发请求
./stress_test.py --clients=100 --requests=1000
```

## 🎯 测试报告

### 生成测试报告
```bash
# 运行所有测试并生成报告
./run_all_tests.sh > test_report.txt 2>&1

# 分析测试结果
grep -E "✅|❌|⚠️" test_report.txt
```

### 验证互操作性矩阵
```bash
# 验证所有通信场景
./validate_interop_matrix.sh
```

## 🤝 贡献

### 添加新测试
1. 在相应目录创建测试文件
2. 更新本指南
3. 验证测试工作正常
4. 提交Pull Request

### 报告问题
1. 描述测试场景
2. 提供重现步骤
3. 包括日志和错误信息
4. 建议修复方案

---

通过本指南，您可以全面测试Python和Rust端之间的互相通信，确保A2A协议在不同语言实现间的互操作性。