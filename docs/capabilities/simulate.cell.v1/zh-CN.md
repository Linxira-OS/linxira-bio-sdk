# 细胞谱系模拟

## 用途

以种子化、精确随机动力学模拟机制型单细胞谱系——Telegraph（双态启动子）基因表达、
Monod 耦合生长、二项分子分配的分裂——为基准测试、方法验证与 GPU 一致性参照提供
确定性合成真值。

## 输入

可选模型 JSON。缺省时运行内置 12 基因面板：默认两小时谱系内呈现 Monod 耦合的
生物量增长与底物耗竭。

## 参数

`--model model.json` 覆盖模型：`monod_constant`（Monod 半饱和常数）、
`substrate_initial`（初始底物单位）、`division_biomass`（分裂阈值）与
`genes[]`（每基因分钟级速率：`kon` 启动子开启、`koff` 关闭、`ks` 转录、
`kdm` mRNA 降解、`ktl` 每 mRNA 翻译、`kdp` 蛋白降解、`yield_per_protein`
每蛋白生物量贡献）。`--time` 设定模拟分钟数（默认 120），`--seed` 设定
种子（默认 42），`--bins` 设定时间序列分箱数（默认 100，范围 1-100000），
`--max-events` 设定 Gillespie 事件预算（默认 20,000,000）。

## 输出

TSV 时间序列（各 bin 的总 mRNA、总蛋白、生物量、Monod 因子均值与分裂计数）
与 JSON 汇总：种子、模型回显、事件/转录/翻译计数、分裂数、谱系深度、底物轨迹、
最终生物量、平均代时，以及逐基因的时间平均与终态表达。

## 示例

```bash
linxira-bio simulate cell lineage.tsv --time 120 --seed 42 --json
linxira-bio simulate cell lineage.tsv --model mymodel.json --time 600 --bins 200 --seed 7 --json
```

## 结果解读

Monod 因子即培养基耗竭曲线：底物充裕时接近 1，谱系消耗底物后塌缩，生长进入
停滞期。平均代时只覆盖分裂窗口（首次到最后一次分裂）。逐基因均值是时间平均的
mRNA/蛋白占据量；表达爆发噪声来自 Telegraph 启动子，相同速率的基因也会分化。
种子 + 模型 + 选项即复现契约：相同输入在任意机器上逐字节再生成 TSV 与汇总——
跨机字节级一致是验收门，不是加分项。

## 注意事项

模型是机制模型，不是拟合的真实生物：单隔室、无体积、无 DNA 复制，蛋白降解不
缩减生物量，分裂只对被追踪的子代做二项分配、姊妹谱系被丢弃。速率常数为示意值
而非实测值；不得与真实菌株的测量结果对比。f64 事件次序满足 IEEE 确定性，本引擎
的跨平台一致性成立。

## 运行时依赖

除 CLI 外无依赖——纯 Rust，xoshiro256** 流上的 Gillespie 精确直接法。

## 引用

方法源流：Gillespie, D. T. *J. Comput. Chem.* **1977**, 28(8), 2347-2357
（精确随机模拟）；双态启动子遵循标准 Telegraph 模型；Monod 生长遵循
Monod, A. *Annu. Rev. Microbiol.* **1949**, 3, 371-394。当模拟谱系支撑已发表
对比时，按版本引用本能力。

## 故障排除

无分裂：增大 `--time`、降低 `division_biomass`，或在模型中提高翻译速率
`ktl`/`yield_per_protein`。事件预算耗尽：提高 `--max-events`、缩短 `--time`
或减少基因数。两次运行或两台机器结果不一致：种子、模型文件或某个选项不同——
用 JSON 汇总的 `model` 回显与台账逐项核对。
