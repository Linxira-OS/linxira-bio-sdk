# plot.render.v1

从 PlotSpec JSON 经原生绘图 pack（matplotlib 或 ggplot2）渲染**确定性、全参数化**的
科研图表。这里的绘图是数据产品，不是图像生成：每个视觉旋钮都是显式、带版本的参数，
同一份 spec 每次渲染出相同字节——这与"让 AI 画一张图"是两回事。

## 用途

把分析结果变成出版物级图表，且图表的一切属性（坐标轴、主题、调色板、尺寸、dpi、
格式、输出文件名）都是契约内显式参数，因此图可复现、可 diff、可在两个独立绘图后端
之间证明数据一致。没有隐藏参数，也没有随机字节。

## 输入

- 请求 JSON：`{"plot_spec": {...}, "output_path": "<文件名>"}`。
- `plot_spec` 须满足 `schemas/plot-spec.schema.json`。支持的数据图型：
  `scatter`、`line`、`bar`（字段 `series`，每系列 `x`/`y`）、`box`、`violin`
  （字段 `groups`，每组 `values`）、`heatmap`（字段 `matrix`：
  `row_labels`/`col_labels`/`values`）。

## 参数

以下字段双后端全部支持（缺省按 PlotSpec schema）：

| 字段 | 缺省 | 说明 |
|---|---|---|
| `title` / `subtitle` | 空 | 标题/副标题 |
| `x_label` / `y_label` | 取 `x_label_hint`/`y_label_hint`，否则 `x`/`y` | 调用方指定或由输入文件名推导 |
| `x_range` / `y_range` | 自动 | 两元素 [min, max] |
| `x_log` / `y_log` | false | 对数轴 |
| `grid` | true | 网格 |
| `theme` | `light` | `dark`、`light`、`publication` |
| `palette` | `set2` | 调色板名，各后端映射原生色系；未知名回退并警告 |
| `legend` | true | 图例 |
| `figure.width/height/dpi` | 800 / 600 / 150 | 尺寸与分辨率 |
| `font.family` / `font.size` | 平台默认 / 12 | 字体 |
| `output.format` | `svg` | `svg`、`png`、`pdf`；产物扩展名随之 |
| `output_path`（请求级） | `figure.svg` | **调用方自定义输出文件名** |

## 输出

- `output_path` 指定的图文件（扩展名替换为请求格式）。
- 渲染结果 JSON（`schemas/plot-render-result.schema.json`）：`backend`、
  `plot_spec_sha256`（溯源）、`artifact`（路径/格式/宽高/dpi/字节）、
  `data_summary`（图型、系列数、每系列点数、x/y 范围）、`warnings`。

## 示例

```bash
linxira-bio workflow run org.linxira.visualization-matplotlib --request request.json --json
```

```bash
python workflows/org.linxira.visualization-matplotlib/src/render.py \
  --request request.json --result result.json
Rscript workflows/org.linxira.visualization-ggplot2/src/render.R \
  --request request.json --result result.json
```

请求示例：

```json
{"plot_spec": {"title": "PCA 得分图", "x_label": "PC1 (38%)",
  "y_label": "PC2 (21%)", "theme": "publication", "output": {"format": "png"},
  "data": {"kind": "scatter", "series": [
    {"name": "对照", "x": [1, 2, 3], "y": [2, 1.5, 3.5]},
    {"name": "处理", "x": [3.5, 4, 5], "y": [4, 5.5, 6]}]}},
 "output_path": "pca-scores.png"}
```

## 结果解读

- `data_summary` 描述图里实际画的数据：系列数、每系列点数、坐标范围。双后端渲染
  同一 spec 时这些必须相等——这就是跨后端数据一致性保证（双 pack 测试均有断言）；
  不承诺像素级一致。
- `plot_spec_sha256` 是各后端内部的重跑稳定性溯源；两种语言 JSON 规范化不同，跨
  后端不要求相等。

## 注意事项

- svg 与 png 跨次运行**字节稳定**：matplotlib 固定 svg hash 盐并抑制日期元数据；
  ggplot2 的 svg 走 svglite、png 走 cairo。ggplot2 的 `pdf` 会嵌入创建时间，已在
  `warnings` 中披露。
- v1 支持六种图型；注释/富集/结构域等专属图型由 Rust SVG 能力覆盖，分子交互查看
  由结构查看器覆盖。
- PlotSpec 的 `interactive` 输出字段已定义但 pack 尚未实现。

## 运行时依赖

- matplotlib pack：CPython 3.12+，matplotlib >= 3.8（pack
  `requirements.lock` 锁定）。
- ggplot2 pack：R >= 4.4，ggplot2、svglite、jsonlite、digest（pack
  `dependencies.lock.json` 锁定）。

## 引用

- Hunter, J. D. (2007). Matplotlib: A 2D graphics environment. Computing in
  Science & Engineering, 9(3), 90–95.
- Wickham, H. (2016). ggplot2: Elegant Graphics for Data Analysis.
  Springer-Verlag New York.

## 故障排除

- `status: "error"` 且提示 unsupported plot kind：`data.kind` 不是六种支持的图型
  之一，修正 spec。
- 未知调色板警告：该名字未映射到此后端的色系，已回退 Set2；改用受支持的名字即可。
- R 渲染器报缺包：按 pack `dependencies.lock.json` 安装（ggplot2、svglite、
  jsonlite、digest）。
