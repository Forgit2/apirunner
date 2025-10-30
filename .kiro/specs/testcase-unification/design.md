# TestCase 统一设计文档

## 概述

本设计文档描述了如何统一 `TestCase` 和 `ExecutableTestCase` 结构体，消除系统中的重复定义，简化架构并提高可维护性。

## 架构

### 当前架构问题

```
test_case_manager.rs          execution.rs
├── TestCase                  ├── ExecutableTestCase
├── RequestDefinition         ├── RequestDefinition  
├── AssertionDefinition       ├── AssertionDefinition
├── VariableExtraction        ├── VariableExtraction
└── AuthDefinition            └── AuthDefinition
                              
需要 TestCase::to_executable() 转换
```

### 目标架构

```
test_case_manager.rs          execution.rs
├── TestCase                  ├── (使用 TestCase)
├── RequestDefinition         ├── (使用 test_case_manager 的定义)
├── AssertionDefinition       ├── (使用 test_case_manager 的定义)
├── VariableExtraction        ├── (使用 test_case_manager 的定义)
└── AuthDefinition            └── (使用 test_case_manager 的定义)

直接使用，无需转换
```

## 组件和接口

### 1. 统一的 TestCase 结构体

**位置：** `src/test_case_manager.rs`

**修改内容：**
- 将 `timeout` 字段类型从 `Option<u64>` 改为 `Option<Duration>`
- 保持所有现有字段，确保存储和执行功能完整

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub protocol: String,
    pub request: RequestDefinition,
    pub assertions: Vec<AssertionDefinition>,
    pub variable_extractions: Option<Vec<VariableExtraction>>,
    pub dependencies: Vec<String>,
    pub timeout: Option<Duration>,  // 修改：从 u64 改为 Duration
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u32,
    pub change_log: Vec<ChangeLogEntry>,
}
```

### 2. 执行引擎接口更新

**位置：** `src/execution.rs`

**修改内容：**
- 移除 `ExecutableTestCase` 结构体
- 移除重复的 `RequestDefinition`、`AssertionDefinition` 等结构体
- 更新 `ExecutionStrategy` trait 使用 `TestCase`
- 导入 `test_case_manager` 中的结构体

```rust
use crate::test_case_manager::{TestCase, RequestDefinition, AssertionDefinition, VariableExtraction, AuthDefinition};

#[async_trait]
pub trait ExecutionStrategy: Send + Sync {
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,  // 修改：使用 TestCase 而不是 ExecutableTestCase
        context: ExecutionContext,
    ) -> Result<ExecutionResult>;

    fn strategy_name(&self) -> &str;
}
```

### 3. VariableExtraction 结构体统一

**当前问题：**
- `test_case_manager::VariableExtraction` 使用 `String` 类型的 source
- `execution::VariableExtraction` 使用 `ExtractionSource` 枚举

**解决方案：**
在 `test_case_manager.rs` 中定义统一的 `ExtractionSource` 枚举：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtractionSource {
    ResponseBody,
    ResponseHeader,
    StatusCode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariableExtraction {
    pub name: String,
    pub source: ExtractionSource,  // 修改：使用枚举而不是字符串
    pub path: String,
    pub default_value: Option<String>,
}
```

### 4. 分布式执行接口更新

**位置：** `src/distributed_execution.rs`

**修改内容：**
- 更新 `add_task` 方法接受 `Vec<TestCase>`
- 移除对 `ExecutableTestCase` 的引用

## 数据模型

### 序列化兼容性

为了保持向后兼容性，我们需要处理现有数据文件中的字段类型差异：

1. **timeout 字段迁移：**
   - 现有文件中 timeout 是数字（秒）
   - 新版本中 timeout 是 Duration 对象
   - 需要自定义 serde 序列化/反序列化

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    // ... 其他字段
    #[serde(
        serialize_with = "serialize_duration_as_secs",
        deserialize_with = "deserialize_duration_from_secs"
    )]
    pub timeout: Option<Duration>,
}

fn serialize_duration_as_secs<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match duration {
        Some(d) => serializer.serialize_some(&d.as_secs()),
        None => serializer.serialize_none(),
    }
}

fn deserialize_duration_from_secs<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let secs: Option<u64> = Option::deserialize(deserializer)?;
    Ok(secs.map(Duration::from_secs))
}
```

2. **VariableExtraction source 字段迁移：**
   - 现有文件中 source 是字符串
   - 新版本中 source 是枚举
   - 需要自定义反序列化逻辑

## 错误处理

### 迁移错误处理

1. **字段类型不匹配：**
   - 提供清晰的错误消息
   - 建议用户如何修复数据文件

2. **未知的 source 类型：**
   - 对于无法识别的 source 字符串，默认使用 `ResponseBody`
   - 记录警告日志

## 测试策略

### 1. 单元测试

- 测试 TestCase 的序列化/反序列化
- 测试字段类型转换的正确性
- 测试向后兼容性

### 2. 集成测试

- 测试执行引擎使用统一 TestCase 的功能
- 测试所有执行策略（串行、并行、分布式）
- 测试现有测试用例文件的加载

### 3. 迁移测试

- 创建包含旧格式数据的测试文件
- 验证系统能够正确加载和转换
- 验证转换后的功能完整性

## 实施计划

### 阶段 1：准备工作
1. 备份现有代码
2. 创建迁移测试用例
3. 更新 TestCase 结构体定义

### 阶段 2：核心修改
1. 修改 `test_case_manager.rs` 中的结构体定义
2. 更新 `execution.rs` 移除重复定义
3. 修改执行引擎接口

### 阶段 3：更新依赖模块
1. 更新 `distributed_execution.rs`
2. 更新导入/导出模块
3. 更新所有测试文件

### 阶段 4：测试和验证
1. 运行所有测试
2. 验证向后兼容性
3. 性能测试

### 阶段 5：清理
1. 移除废弃的代码
2. 更新文档
3. 清理导入语句

## 风险和缓解措施

### 风险 1：破坏现有功能
**缓解措施：**
- 全面的测试覆盖
- 分阶段实施
- 保持现有 API 兼容性

### 风险 2：数据迁移失败
**缓解措施：**
- 提供数据迁移工具
- 详细的错误消息和修复建议
- 支持多种数据格式

### 风险 3：性能影响
**缓解措施：**
- 性能基准测试
- 优化序列化/反序列化逻辑
- 监控内存使用

## 成功标准

1. **功能完整性：** 所有现有功能继续正常工作
2. **代码简化：** 移除所有重复的结构体定义
3. **测试通过：** 所有现有测试和新增测试都通过
4. **向后兼容：** 现有数据文件能够正确加载
5. **性能保持：** 执行性能不下降