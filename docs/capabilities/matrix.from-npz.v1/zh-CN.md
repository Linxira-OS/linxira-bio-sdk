# matrix.from-npz.v1

把 NumPy `.npz` 归档（`np.savez` 或 `np.savez_compressed` 产物）中的主
2-D 矩阵导入为带行列标签的普通 CSV/TSV 表。引擎在 Rust 内原生读取
ZIP 容器与每个 `.npy` v1.0 成员——不依赖 Python 运行时——从而让仅有
npz 产物的计数矩阵也能进入分隔表能力链（差异表达、归一化、PCA、聚类）。

## 用途

把仅有 npz 形态的表达/计数矩阵转换为 SDK 其余能力消费的分隔表格式；
归档自带标签时保留标签，缺失时生成位置标签并以 warning 披露。

## 输入

- 一个 `.npz` 归档：`np.savez`（ZIP_STORED）或 `np.savez_compressed`
  （DEFLATE）写出的 `.npy` 成员 ZIP 包。
- 主矩阵必须是 dtype 为 `<f8`、`<f4`、`<i8` 或 `<i4` 的 2-D 数组；当其
  名为 `counts` 或 `matrix`，或是归档中唯一的 2-D 条目时自动选中。
- 可选 1-D 标签数组：`rows` 或 `row_labels` 作行标签；`cols`、
  `col_labels` 或 `genes` 作列标签。标签 dtype 可为上述数值型、字节串
  （`|S`）或 unicode（`<U`）。数组名匹配时会去掉 `.npy` 后缀。

## 参数

- `--matrix-name NAME`：导入指定的 2-D 数组，替代自动选择（缺失时列出
  可用数组并报错）。
- `--row-labels NAME`：用指定 1-D 数组作行标签。
- `--col-labels NAME`：用指定 1-D 数组作列标签。
- `--json`：输出标准结果信封。
- 输出分隔符由输出扩展名决定：`.csv` 逗号、`.tsv` 制表符；其他扩展名
  一律拒绝。

## 输出

- `--output counts.csv|counts.tsv`：矩阵表。表头为 `row` 加各列标签；
  每个数据行为行标签加该行数值（最短往返表示）。
- 摘要字段：input_arrays（每个成员的名字、shape、dtype）、
  matrix_array、rows、cols、cell_count、output_path、output_bytes、
  warnings。

## 示例

```bash
linxira-bio matrix from-npz counts.npz counts.tsv --json
linxira-bio matrix from-npz scanpy_export.npz matrix.csv --matrix-name X --row-labels genes --col-labels barcodes
```

## 结果解读

- `matrix_array` 标明实际导入的条目；与导出脚本对照 `input_arrays`
  可确认选中的就是目标矩阵。
- 标签缺失或长度不匹配时回退为位置标签 `row0..rowN-1` /
  `col0..colM-1`，且始终以 warning 披露——绝不静默处理。
- `fortran_order: True` 的成员在导入时转置为 C（行主序），行列语义与
  numpy 保持一致。

## 注意事项

- 仅支持 npy 1.x 格式、小端 dtype `<f8`/`<f4`/`<i8`/`<i4`（标签另支持
  `|S`/`<U`）以及 ZIP STORED/DEFLATE 条目；结构化 dtype、大端数据、
  3-D 数组、ZIP64 归档与非 npy 成员都会显式报错。
- 数值经 `f64` 写出；超过 2^53 的 `i64` 计数会损失整数精度，`<f4`
  值可能以 f64 展开的小数打印。
- 重复或空标签以 warning 披露（空标签替换为位置标签），因为下游矩阵
  消费方会拒绝它们。
- 本能力是格式导入而非质控；做差异分析前请先对产出的表跑
  `expression matrix-qc`。

## 运行时依赖

- 仅需 SDK 二进制；ZIP 与 npy 解析全部由 Rust（csv、flate2）实现，
  不涉及 Python、numpy 或 shell。

## 引用

- Harris, C. R. et al. (2020). Array programming with NumPy. Nature, 585,
  357–362.
- Collette, A. (2013). Python and HDF5 / NumPy binary format（`.npy`
  1.0 版规范）. NumPy documentation.

## 故障排除

- `no 2-D matrix array found` / `multiple 2-D arrays found`：把矩阵条目
  命名为 `counts` 或 `matrix`，或显式传 `--matrix-name`。
- `unsupported dtype`：先在 numpy 中转换（例如
  `X.astype('<f8')`）再导出；结构化或大端 dtype 无法导入。
- 标签长度不匹配的 warning：标签数组没有覆盖矩阵的对应轴（常见于
  genes×samples 矩阵中 `genes` 实为行标签却被按列匹配）；重新导出时
  改名，或传 `--row-labels`/`--col-labels`。
- `output must end in .csv or .tsv`：重命名输出文件；分隔符按扩展名
  选择。
