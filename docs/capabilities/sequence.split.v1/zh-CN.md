# FASTA 拆分

## 用途

将单个 FASTA 按确定性编号拆成多个分块文件，适用于批处理、上传限制或下游工具分批运行。

## 输入

一个可读取的 FASTA 文件。支持纯文本和 gzip 流。

## 参数

命令需要输入 FASTA 和输出目录。可选参数包括 `--records-per-file` 和 `--prefix`。`--json` 返回标准结果封装。

## 输出

在输出目录中写入类似 `part_001.fa` 的编号 FASTA 分块。JSON 返回输入记录数、输出文件数、残基数、每文件记录数和前缀。

## 示例

```bash
linxira-bio sequence split input.fa chunks --records-per-file 1000 --prefix part --json
```

## 结果解读

分块编号是确定性的，并保持输入记录顺序。

## 注意事项

已有分块文件不会被覆盖。重复运行时请选择空目录或新的前缀。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

FASTA 分块输出遵循保留记录顺序的常见批处理行为。

## 故障排除

如果命令拒绝写入分块，请删除旧输出文件或选择新的输出目录。
