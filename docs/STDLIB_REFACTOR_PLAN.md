# stdlib 模块化拆分计划

## 目标
将 stdlib/mod.rs (14,017 行) 中的大型模块拆分为独立文件，避免单文件膨胀。

## 拆分原则
- **一个原生库一个单元**
- 超过 300 行的模块优先拆分
- stdlib/mod.rs 仅保留模块路由和分发逻辑

## 待拆分模块列表

| 优先级 | 模块 | 行数 | 目标路径 | 状态 |
|-------|------|------|---------|-----|
| 🔴 1 | time | 1,680 | stdlib/time.rs | ⏳ 待拆分 |
| 🔴 2 | cli | 695 | stdlib/cli.rs | ⏳ 待拆分 |
| 🔴 3 | crypto | 594 | stdlib/crypto.rs | ⏳ 待拆分 |
| 🟡 4 | ws_server | 517 | stdlib/net/ws_server.rs | ⏳ 待拆分 |
| 🟡 5 | db | 406 | stdlib/db.rs | ⏳ 待拆分 |
| 🟡 6 | http_server | 393 | stdlib/net/http_server.rs | ⏳ 待拆分 |
| 🟡 7 | csv | 391 | stdlib/encoding/csv.rs | ⏳ 待拆分 |
| 🟡 8 | mail | 384 | stdlib/mail.rs | ⏳ 待拆分 |
| 🟡 9 | fs | 379 | stdlib/fs.rs | ⏳ 待拆分 |
| 🟡 10 | url | 366 | stdlib/url.rs | ⏳ 待拆分 |
| 🟡 11 | semver | 361 | stdlib/semver.rs | ⏳ 待拆分 |
| 🟡 12 | random | 345 | stdlib/random.rs | ⏳ 待拆分 |
| 🟡 13 | env | 326 | stdlib/env.rs | ⏳ 待拆分 |

**总计**: 13 个模块，约 **6,637 行** 将被拆分

## 目标目录结构

```
stdlib/
├── mod.rs              # 主模块（仅路由，~7,000 行）
├── time.rs             # 1,680 行
├── cli.rs              # 695 行
├── crypto.rs           # 594 行
├── db.rs               # 406 行
├── mail.rs             # 384 行
├── fs.rs               # 379 行
├── url.rs              # 366 行
├── semver.rs           # 361 行
├── random.rs           # 345 行
├── env.rs              # 326 行
├── encoding/
│   └── csv.rs          # 391 行
├── net/
│   ├── ws_server.rs    # 517 行
│   └── http_server.rs  # 393 行
└── gtp/                # 已完成 ✅
    ├── mod.rs
    ├── client.rs
    └── server.rs
```

## 拆分步骤（每个模块）

1. 创建独立文件 `stdlib/{module}.rs`
2. 从 `mod.rs` 中提取 `{module}_module()` 函数及相关辅助函数
3. 在 `stdlib/mod.rs` 中：
   - 添加 `pub mod {module};`
   - 修改 `load_native_module()` 为 `{module}::{module}_module()`
4. 编译测试验证

## 预期收益

- **代码行数**: stdlib/mod.rs 从 14,017 → ~7,000 行（减少 50%）
- **可维护性**: 每个模块独立文件，职责清晰
- **可扩展性**: 添加新功能不再污染主文件
- **编译速度**: 模块化有助于增量编译

## 实施策略

### 阶段 1：超大模块（优先级 🔴）
- time (1,680 行)
- cli (695 行)
- crypto (594 行)

**减负**: ~3,000 行

### 阶段 2：大型模块（优先级 🟡）
- ws_server, db, http_server, csv, mail
- fs, url, semver, random, env

**减负**: ~3,600 行

### 阶段 3：测试和优化
- 运行完整测试套件
- 清理未使用的导入
- 更新文档

## 注意事项

1. **保持 API 兼容性**: 模块拆分不影响 `load_native_module()` 的行为
2. **依赖关系**: 某些辅助函数可能被多个模块共享，需要提取到共同位置
3. **测试覆盖**: 每次拆分后运行测试确保功能正常
4. **增量提交**: 每拆分 2-3 个模块提交一次，避免风险

## 开始实施

从最大的 **time** 模块开始...
