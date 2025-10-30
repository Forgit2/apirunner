# TestCase 统一实施计划

- [x] 1. 更新 TestCase 结构体定义

  - 修改 test_case_manager.rs 中的 TestCase 结构体，将 timeout 字段从 Option<u64> 改为 Option<Duration>
  - 添加自定义 serde 序列化/反序列化函数处理 timeout 字段的向后兼容性
  - 更新 VariableExtraction 结构体，将 source 字段从 String 改为 ExtractionSource 枚举
  - 在 test_case_manager.rs 中定义 ExtractionSource 枚举
  - _需求: 3.1, 3.2_

- [x] 2. 移除 execution.rs 中的重复结构体定义

  - 删除 ExecutableTestCase 结构体定义
  - 删除 execution.rs 中重复的 RequestDefinition、AssertionDefinition、VariableExtraction、AuthDefinition 结构体
  - 更新 execution.rs 的导入语句，从 test_case_manager 导入这些结构体
  - _需求: 5.1, 5.2_

- [x] 3. 更新执行引擎接口

  - 修改 ExecutionStrategy trait 的 execute 方法，将参数从 Vec<ExecutableTestCase> 改为 Vec<TestCase>
  - 更新 SerialExecutor 的 execute 方法实现
  - 更新 ParallelExecutor 的 execute 方法实现
  - 修改 filter_independent_tests 方法的参数和返回类型
  - _需求: 1.2, 2.1, 2.2_

- [x] 4. 更新分布式执行模块

  - 修改 distributed_execution.rs 中的 add_task 方法，将参数从 Vec<ExecutableTestCase> 改为 Vec<TestCase>
  - 更新相关的导入语句
  - _需求: 2.3_

- [x] 5. 移除 TestCase::to_executable 转换方法

  - 删除 test_case_manager.rs 中的 to_executable 方法
  - 清理所有调用 to_executable 方法的代码
  - _需求: 5.3_

- [x] 6. 更新 lib.rs 导出

  - 修改 lib.rs 中的导出语句，移除 ExecutableTestCase 的导出
  - 确保 TestCase 及相关结构体正确导出
  - _需求: 5.2_

- [x] 7. 修复所有测试文件

  - 更新 tests/execution_tests.rs，修复 TestCase 结构体初始化
  - 添加缺失的字段（tags, protocol, created_at, updated_at, version, change_log）
  - 修正字段类型不匹配问题（timeout 使用 Duration，source 使用枚举）
  - 更新导入语句使用正确的结构体定义
  - _需求: 1.1, 1.3_

- [x] 8. 更新其他相关模块

  - 检查并更新 import_export 模块中对 TestCase 的使用
  - 检查并更新 interactive_execution 模块中的相关代码
  - 检查并更新 real_time_monitoring 模块中的相关代码
  - _需求: 4.3_

- [x] 9. 创建数据迁移测试

  - 创建包含旧格式 timeout 字段（u64）的测试数据文件
  - 创建包含旧格式 source 字段（String）的测试数据文件
  - 编写测试验证向后兼容性
  - _需求: 4.1, 4.2_

- [x] 10. 运行完整测试套件

  - 执行所有单元测试，确保没有编译错误
  - 执行所有集成测试，验证功能完整性
  - 执行性能测试，确保性能不下降
  - _需求: 2.1, 2.2, 2.3, 2.4_

- [x] 11. 清理和优化

  - 移除所有未使用的导入语句
  - 清理注释中提到 ExecutableTestCase 的地方
  - 优化代码结构，确保一致性
  - _需求: 5.4_

- [x] 12. 验证向后兼容性

  - 测试加载现有的测试用例配置文件
  - 验证导入/导出功能正常工作
  - 确保 API 调用的兼容性
  - _需求: 4.1, 4.2, 4.3_