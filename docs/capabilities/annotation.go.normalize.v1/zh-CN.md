# GO 注释归一化

## 用途

从本地 CSV/TSV 注释列构建确定性的基因到 GO 术语关联表。

## 输入

带表头的 CSV/TSV；包含基因标识列和以逗号、分号或竖线分隔的 `GO:` 标识列。

## 参数

自动表头别名不匹配时使用 `--gene-column` 和 `--go-column`；必须提供输出 TSV 路径。

## 输出

JSON 返回行数、基因数、术语数和去重关联数，并写出 `gene_id`、`term_id`、`term_name`、`namespace` TSV；拒绝覆盖已有文件。

## 示例

```bash
linxira-bio annotation go annotations.tsv go-associations.tsv --json
```

## 结果解读

每行是一条唯一的基因到 GO 关联；重复来源行不会增加关联数。

## 注意事项

仅验证 GO 标识语法，不下载本体、不处理废弃术语，也不自动展开父级术语。

## 运行时依赖

解析、校验、去重和 TSV 写出均为本地 Rust。

## 引用

记录注释来源、本体版本、标识映射和输入筛选规则。

## 故障排除

确认输入有表头；非标准列名请显式指定基因列和 GO 列。
