# Motif 序列 Logo

## 用途

从 MEME motif 矩阵在本地生成 SVG 序列 logo。

## 输入

包含 `ALPHABET` 和有限 `letter-probability matrix` 的 MEME 文本。

## 参数

当前版本没有可选参数。

## 输出

SVG，以及请求时的 JSON 结果信封。

## 示例

```text
linxira-bio motif logo motif.meme motif.svg --json
```

## 结果解读

字母高度反映输入的位置概率。

## 注意事项

渲染器使用第一个有效矩阵，不执行 motif 发现。

## 运行时依赖

仅使用本地 Rust。

## 引用

应引用生成 MEME 矩阵的 motif 发现方法。

## 故障排除

请导出标准 MEME 文本，确保字母表和矩阵行宽匹配。
