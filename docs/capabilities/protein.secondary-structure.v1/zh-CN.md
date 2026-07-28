# 蛋白二级结构注释

## 用途

对本地 PDB 或 mmCIF 坐标运行 DSSP，生成逐残基二级结构注释。

## 输入

输入一个本地 PDB 或 mmCIF 坐标文件。

## 参数

当前版本没有科学调参项。

## 输出

生成 DSSP 文本和受控执行元数据。

```bash
linxira-bio protein secondary-structure model.cif model.dssp --json
```

## 示例

上面的命令在源结构旁生成逐残基 DSSP 文本。

## 结果解读

DSSP 是从坐标推导的注释；缺失残基、替代构象和不完整坐标都会影响结果。

## 注意事项

本包装层不会修复缺失坐标，也不会自动选择生物学组装体。

## 运行时依赖

需要 `PATH` 中的 `mkdssp`，或设置 `LINXIRA_BIO_MKDSSP`；结构文件不会上传。

## 引用

引用 DSSP、版本以及坐标结构标识符和版本。

## 故障排除

审计 `mkdssp`；程序不在 `PATH` 时设置 `LINXIRA_BIO_MKDSSP`。
