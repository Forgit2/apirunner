# TestCase 和 ExecutableTestCase 统一需求文档

## 介绍

当前系统中存在两个重复的测试用例结构体：`TestCase`（在 test_case_manager 中）和 `ExecutableTestCase`（在 execution 中）。这种重复导致了类型不匹配、转换复杂性和维护困难。本需求文档旨在统一这两个结构体，简化系统架构。

## 需求

### 需求 1：统一测试用例数据结构

**用户故事：** 作为开发者，我希望系统中只有一个测试用例数据结构，这样我就可以在整个系统中一致地使用它，而不需要进行复杂的类型转换。

#### 验收标准

1. WHEN 系统需要表示测试用例时 THEN 系统应该使用统一的 TestCase 结构体
2. WHEN 执行引擎需要执行测试时 THEN 执行引擎应该直接接受 TestCase 而不是 ExecutableTestCase
3. WHEN 测试代码创建测试用例时 THEN 测试代码应该能够直接使用 TestCase 而无需类型转换

### 需求 2：保持执行功能完整性

**用户故事：** 作为系统用户，我希望在统一数据结构后，所有现有的执行功能都能正常工作，包括串行执行、并行执行和分布式执行。

#### 验收标准

1. WHEN 使用统一的 TestCase 结构体时 THEN 串行执行器应该能够正常执行测试
2. WHEN 使用统一的 TestCase 结构体时 THEN 并行执行器应该能够正常执行测试
3. WHEN 使用统一的 TestCase 结构体时 THEN 分布式执行器应该能够正常执行测试
4. WHEN 执行测试时 THEN 所有现有的功能（变量提取、依赖管理、断言等）应该继续工作

### 需求 3：简化字段类型和结构

**用户故事：** 作为开发者，我希望 TestCase 结构体中的字段类型是一致和合理的，这样我就可以避免类型转换错误。

#### 验收标准

1. WHEN TestCase 包含 timeout 字段时 THEN timeout 字段应该使用 Duration 类型而不是 u64
2. WHEN TestCase 包含嵌套结构体时 THEN 这些结构体应该来自同一个模块，避免重复定义
3. WHEN 系统需要序列化/反序列化 TestCase 时 THEN 所有字段都应该支持 serde

### 需求 4：保持向后兼容性

**用户故事：** 作为现有系统的用户，我希望在统一数据结构后，我的现有配置和数据文件仍然能够正常工作。

#### 验收标准

1. WHEN 加载现有的测试用例文件时 THEN 系统应该能够正确解析和加载
2. WHEN 现有的 API 调用使用旧的结构体时 THEN 系统应该提供平滑的迁移路径
3. WHEN 导入/导出功能使用测试用例时 THEN 功能应该继续正常工作

### 需求 5：清理冗余代码

**用户故事：** 作为维护者，我希望移除所有重复的结构体定义和转换代码，这样系统就更容易维护和理解。

#### 验收标准

1. WHEN 统一完成后 THEN ExecutableTestCase 结构体应该被移除
2. WHEN 统一完成后 THEN execution 模块中重复的 RequestDefinition、AssertionDefinition 等应该被移除
3. WHEN 统一完成后 THEN TestCase::to_executable() 方法应该被移除
4. WHEN 统一完成后 THEN 所有相关的转换代码应该被清理