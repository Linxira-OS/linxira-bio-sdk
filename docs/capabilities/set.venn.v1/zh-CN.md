# 精确 Venn 区域分析

## 用途

统计 2–6 个生物标识符集合的精确成员区域。

## 输入

本地 CSV/TSV 表格；表头是集合名称，每个非空单元格表示该列集合中的一个标识符。同一集合内的重复标识符会去重。

## 参数

- `--include-items`：额外返回每个精确区域中的标识符（worker 契约参数名
  `include_items`）；计数始终返回。
- `--backend auto|rust|python|r`：实现后端。`rust`（及缺省）运行原生引擎；
  `python` 和 `r` 经 worker 路由到 benchmark 包，按同样的精确集合运算复刻
  （不涉及任何绘图库），计数完全一致。`auto` 查询
  `runtime-preferences.json`：命中非 rust 后端时输出
  `backend_from_preferences` 警告。
- `--json`：输出完整结果封装。

## 输出

JSON 返回各集合大小、并集大小和所有观察到的精确交集。精确交集中的项目只存在于列出的集合，不包含同时属于其他集合的项目。

## 示例

```bash
linxira-bio set venn sets.tsv --json
```

## 结果解读

注意区分精确区域和包含式两两交集。例如 `A ∩ B` 精确区域不包含同时存在于 `C` 的标识符。

## 注意事项

Venn 最多接受 6 列；输入最多 100 万行、100 万个唯一标识符。默认不返回项目列表，以限制 JSON 大小。

## 运行时依赖

- `rust`（默认）：解析、去重和精确成员统计均在本地 Rust 中运行。
- `python` / `r`：对应的 benchmark 包；两者均以零第三方依赖复刻集合运算。
  均无网络访问。

## 引用

应引用每个生物集合的数据来源和筛选规则。

## 故障排除

- 确认首行包含唯一且非空的集合名，并使用正确的 `.csv` 或 `.tsv` 扩展名。
- `unknown --backend value`：该参数只接受 `auto`、`rust`、`python`、`r`。
