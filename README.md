# A2A Protocol Rust Implementation

这是 A2A (Agent-to-Agent) 协议的 Rust 实现，致力于复刻 Python 版本 [a2a-python](https://github.com/a2aproject/a2a-python) 的功能。该实现使 AI 代理和客户端能够使用明确定义的数据结构集和消息类型进行通信。

## 🎯 项目目标

本项目旨在提供一个与 [a2a-python](https://github.com/a2aproject/a2a-python) API 兼容的 Rust 版本，实现：

- **协议对齐**: 100% 符合 A2A 协议规范
- **Python 兼容**: 与 a2a-python 完全互操作
- **类型安全**: 利用 Rust 类型系统提供编译时安全保证
- **高性能**: 内存安全，零运行时开销
- **生态集成**: 融入 Rust 生态系统

## 📊 当前进度

| 模块 | 对齐度 | 状态 |
|------|--------|------|
| 核心类型系统 | **100%** | ✅ 完全对齐 |
| 认证系统 | **100%** | ✅ 完全对齐 |
| 工具函数 | **100%** | ✅ 完全对齐 |
| 客户端实现 | **72%** | ⚠️ JSON-RPC 已实现，REST/gRPC 待完成 |
| 服务端实现 | **50%** | ⚠️ 基础功能已实现 |
| **总体** | **74%** | ⚠️ 良好对齐 |

详细的对比分析请参考 [docs/comprehensive_python_rust_comparison.md](docs/comprehensive_python_rust_comparison.md)。

## ✨ 主要特性

### 🔄 Python 兼容性
- **完全互操作**: 与 a2a-python 客户端/服务端无缝通信
- **序列化兼容**: JSON 格式与 Python 版本 100% 匹配
- **API 对齐**: 核心接口与 Python 版本保持一致

### 🛡️ 类型安全
- **编译时检查**: Rust 类型系统防止运行时错误
- **内存安全**: 无需垃圾回收，无数据竞争
- **零成本抽象**: 高性能与安全性兼备

### 🔐 完整认证系统
```rust
// 支持所有主流认证方案
CredentialService:
├── InMemoryContextCredentialStore  // 内存凭据存储
├── EnvironmentCredentialService     // 环境变量凭据
└── CompositeCredentialService       // 组合凭据服务

AuthInterceptor:
├── Bearer Token ✅
├── API Key (Header/Query/Cookie) ✅
├── OAuth2 ✅
├── OIDC ✅
└── mTLS ✅
```

### 🏭 工厂模式客户端
```rust
use a2a_rust::a2a::client::factory::ClientFactory;

// 一行代码创建客户端
let client = ClientFactory::connect(
    "https://agent.example.com".to_string(),
    None, None, None, None, None, None, None
).await?;

// 与 Python 版本使用方式完全一致
```

## 🏗️ 架构设计

### 核心组件
```
src/a2a/
├── core_types.rs          # 核心数据类型 (100% 对齐 a2a-python)
├── models.rs              # 复杂模型 (Message, Task, Artifact)
├── client/                # 客户端实现
│   ├── factory.rs         # ClientFactory (已实现)
│   ├── base_client.rs     # BaseClient (已实现)
│   ├── auth/              # 认证系统 (100% 对齐)
│   └── transports/        # 传输层 (JSON-RPC 已实现)
├── server/                # 服务端实现
│   ├── request_handlers/  # 请求处理器
│   ├── tasks/             # 任务管理
│   └── events/            # 事件系统
└── utils/                 # 工具函数 (100% 对齐)
```

### 数据模型 (与 a2a-python 完全对齐)

| 类型 | Rust | Python | 对齐状态 |
|------|------|--------|----------|
| 消息 | `Message` | `Message` | ✅ 100% |
| 任务 | `Task` | `Task` | ✅ 100% |
| 工件 | `Artifact` | `Artifact` | ✅ 100% |
| 部件 | `Part` | `Part` | ✅ 100% |
| 角色 | `Role` | `Role` | ✅ 100% |
| 状态 | `TaskState` | `TaskState` | ✅ 100% |

## 🚀 快速开始

### 安装依赖
```bash
# 本项目依赖 (自动处理)
# serde, serde_json, uuid, url, chrono, thiserror
```

### 基础使用

#### 1. 创建客户端
```rust
use a2a_rust::a2a::client::factory::ClientFactory;
use a2a_rust::a2a::client::auth::{AuthInterceptor, InMemoryContextCredentialStore};
use a2a_rust::a2a::core_types::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 设置认证
    let mut store = InMemoryContextCredentialStore::new();
    store.add_credential("bearerAuth", "your-token-here");
    
    let auth_interceptor = AuthInterceptor::new(Arc::new(store));
    let interceptors = vec![Box::new(auth_interceptor)];
    
    // 创建客户端 (与 Python a2a-sdk 使用方式一致)
    let client = ClientFactory::connect(
        "https://your-agent.com".to_string(),
        None,
        None,
        Some(interceptors),
        None,
        None,
        None,
        None,
    ).await?;
    
    Ok(())
}
```

#### 2. 发送消息
```rust
use a2a_rust::a2a::core_types::*;
use serde_json;

// 创建消息 (格式与 Python 版本完全一致)
let message = Message::new(
    Role::User,
    vec![
        Part::text("Hello from Rust client!".to_string()),
        Part::data(serde_json::json!({
            "client": "rust",
            "protocol": "a2a"
        })),
    ],
).with_context_id("session-123".to_string());

// 发送消息
let mut response_stream = client.send_message(message).await?;
while let Some(event) = response_stream.next().await {
    println!("Received: {:?}", event);
}
```

#### 3. 任务管理
```rust
use a2a_rust::a2a::models::*;
use uuid::Uuid;

// 创建任务
let task = Task::new(
    "session-123".to_string(),
    TaskStatus::new(TaskState::Submitted)
).with_metadata(TaskMetadata {
    task_id: Some(Uuid::new_v4().to_string()),
    ..Default::default()
});

// 转换为 TaskOrMessage
let task_or_message = TaskOrMessage::Task(task);
```

## 🔄 Python-Rust 互操作

### 完全兼容的场景
- ✅ JSON-RPC 协议通信
- ✅ 认证流程
- ✅ 消息序列化/反序列化
- ✅ 任务生命周期管理
- ✅ 错误处理

### 示例：Python 客户端 ↔ Rust 服务器
```python
# Python 客户端 (a2a-python)
from a2a.client.client_factory import ClientFactory

client = await ClientFactory.connect("http://localhost:8080")
response = await client.send_message(message)
```

```rust
// Rust 服务器 (a2a-rust)
// 使用内置的 JSON-RPC 处理器
// 完全兼容 Python 客户端的请求格式
```

## 🧪 测试

### 运行所有测试
```bash
cargo test
```

### 兼容性测试 (验证 Python 对齐)
```bash
# 运行与 Python a2a-sdk 的兼容性测试
cargo test --test parts_compatibility_test

# 运行互操作性测试
cargo test --test interop_test
```

### 示例测试
```bash
# 运行 Python 示例 (需要安装 a2a-python)
cd examples
python python_client.py

# 运行 Rust 示例
cargo run --example rust_client
cargo run --example rust_server
```

## 📚 文档

- [类型对齐状态](docs/python_rust_type_alignment_summary.md) - 与 a2a-python 的详细类型对比
- [功能对比分析](docs/comprehensive_python_rust_comparison.md) - 当前实现状态和差异分析
- [认证系统实现](docs/client_authentication_implementation_summary.md) - 认证机制详细说明

## 🔧 开发状态

### ✅ 已完成
- 核心类型系统 (100% 对齐 a2a-python)
- 认证拦截器系统 (100% 对齐 a2a-python)
- JSON-RPC 客户端传输层
- 工厂模式客户端创建
- 基础服务端请求处理
- 完整的工具函数库

### 🚧 进行中
- REST 传输层实现
- gRPC 传输层实现
- 服务端应用框架集成 (Axum)
- 推送通知系统

### 📋 计划中
- 数据库集成 (SQLx)
- Web 应用示例
- 性能基准测试
- 更多传输协议支持

## 🤝 贡献

欢迎贡献！请参考以下指南：

1. **API 对齐**: 确保新功能与 a2a-python 保持一致
2. **测试覆盖**: 包含兼容性测试验证 Python 对齐
3. **文档更新**: 更新相关文档说明当前状态
4. **代码风格**: 遵循 Rust 最佳实践

## 📄 许可证

本项目遵循与原始 A2A 协议实现相同的许可证。

## 🔗 相关链接

- [A2A Protocol Specification](https://github.com/a2aproject/a2a-python)
- [a2a-python](https://github.com/a2aproject/a2a-python) - Python 原始实现
- [类型对齐文档](docs/python_rust_type_alignment_summary.md)
- [功能对比文档](docs/comprehensive_python_rust_comparison.md)

---

**注意**: 本项目正在积极开发中，旨在提供与 a2a-python 完全兼容的 Rust 实现。当前版本支持核心功能和基础互操作性，高级功能正在逐步完善中。
