# 数据归算网络协议（Data Imputation Network Protocol, DIN）— 草案 v0.1

> 本文件是 DIN 作为**协议**的规范草案：定义数据模型、事件 schema、归因与归算算法、身份与账本要求。任何独立实现只要遵守本规范即可互通。
> 产品形态「数据策展网络（DCN）」是本协议的一个实现/托管方，不属于协议本身。
> 配套文档：`Spec.md`（机制与原理）。

---

## 0. 协议范围

DIN 协议只规定**规则与接口**，不规定实现技术：

- ✅ 协议定义：数据模型、事件类型、归因规则、归算算法、签名与可验证性要求、争议流程；
- ❌ 协议不定义：存储用什么数据库、支付走法币还是代币、前端用什么框架、谁来运营。

本文件是协议"可执行"层面的规范（数据模型、事件 schema、归因与归算算法、身份与账本要求）。

**符号约定**

- 折扣 / 归因保留份额：$\lambda \in (0,1)$
- 归因权重矩阵：$\alpha$
- 归算价值向量：$V$

---

## 1. 数据模型

### 1.1 Dataset（数据集）

```
Dataset {
  id            : DatasetID        // 内容寻址（对数据内容哈希派生，防篡改）
  owner         : AgentID
  schema        : SchemaRef        // 数据结构声明
  version       : Version          // 语义化版本，派生产生新 id
  state         : ContentHash      // 数据内容本身的哈希
  created_at    : Timestamp
}
```

- `id` 必须是内容寻址的（例如 `hash(state || schema || version)`），以保证"同一内容 = 同一 id"，从机制上阻止改头换面重复发布同一份数据。

### 1.2 Lineage DAG（谱系图）

```
Edge {
  child  : DatasetID    // 派生出的数据集
  parent : DatasetID    // 被派生的来源数据集
}

G = (V, E)   // 有向无环图；不允许环（派生链上不能回到祖先）
```

- 每个派生事件在图中添加多条边（一个 `child` 对应多个 `parent`）。
- **DAG 约束**：`child` 不得出现在自身的祖先闭包中，否则事件无效。

### 1.3 Payment（支付义务）

每条边挂一笔支付：

```
Payment {
  edge_id      : EdgeID
  amount       : Amount        // 本次订阅支付的金额
  currency     : CurrencyRef
  paid_by      : AgentID       // 策展者（child 的所有者）
  received_by  : AgentID       // parent 的所有者
  settled_at   : Timestamp     // 结算时间（可滞后于事件）
}
```

---

## 2. 事件 Schema（Event Log）

协议的事件日志是**追加写、不可变**的。所有状态（谱系、支付、归算值）都是事件流的派生投影。

### 2.1 事件信封（Envelope）

```
Envelope {
  event_id    : EventID          // 唯一、单调递增
  type        : EventType        // 见 2.2
  payload     : Payload
  authored_by : AgentID
  sig         : Signature        // 对 payload 的签名
  prev_hash   : Hash             // 前一个事件的哈希（链式，防重排）
  merkle_root : Hash             // 可选的批量承诺根
}
```

### 2.2 事件类型

| 类型 | payload 关键字段 | 说明 |
|---|---|---|
| `contribute` | `dataset`, `owner`, `schema`, `state` | 发布新数据集（根节点） |
| `subscribe` | `child`, `parents[]`, `price` | 订阅来源并派生新数据集；等价于"引用 = 策展 = 付费" |
| `payment` | `edge_id`, `amount` | 对已声明订阅的支付结算（可与 `subscribe` 合并，亦可分拆） |
| `attribute` | `edge_id`, `weights[]` | 归因权重的声明/确认（见 §3.1） |
| `challenge` | `target`, `reason`, `deposit` | 对疑似造假/虚假血缘的争议（见 §6） |

**不可变约束**：事件一经写入不得修改或删除；修正必须通过新事件（如 `challenge` → `resolve`）。

---

## 3. 归因规则（Attribution）

### 3.1 归因权重 $\alpha$

对每个派生事件 `subscribe(child, parents, price)`：

- 每笔支付 `price` 按固定规则拆分：
  - **协议留存** $\mu$：销毁或用于公共品，不可回收（防自我交易的关键）；
  - **策展边际**：$1 - \lambda$ 归 `child` 的直接 parent 所有者；
  - **上游归因**：$\lambda$ 沿 parent 的谱系按权重 $\alpha$ 继续向上游分配，其中 $\alpha$ 满足 $\sum_{v \in ancestors(parent)} \alpha_{v,parent} \le 1$。

默认 $\alpha$ 取"按路径长度均匀衰减"或"按贡献占比"两种之一；规则版本化，由治理决定（协议外）。

**伪代码：归因分配**

```
function attribute(payment, parent):
    pool = payment.amount
    retained = pool * mu                      # 协议留存（不可回收）
    pool -= retained
    curator_margin = pool * (1 - lambda)      # parent 所有者保留
    remit_to(parent) += curator_margin
    pool *= lambda                            # 进入上游池
    for ancestor in ancestors(parent):
        share = alpha(ancestor, parent) * pool
        remit_to(ancestor) += share
```

### 3.2 归算算法（Value Imputation）

归算价值 $V$ 是如下线性系统的唯一解（DAG 上唯一且可线性计算）：

$$
V(v) = R_v + \sum_{v' \in \mathcal{C}(v)} \alpha_{v,v'} \left[ p_{v'} + V(v') \right]
$$

**伪代码（拓扑序，$O(|V|+|E|)$）**

```
function impute(G, R, P, alpha):
    order = topological_sort(G)          # 逆拓扑：叶子在前
    V = {}
    for v in order:                      # 从叶子向上
        downstream = 0
        for child in children(v):
            downstream += alpha[v, child] * (P[child] + V[child])
        V[v] = R[v] + downstream
    return V
```

**实现要求**：任何独立实现对同一事件流必须得到**相同的 $V$**（确定性）。因此 $\alpha$、$R$、$P$ 的取值规则必须在协议内明确到可复现。

---

## 4. 身份与签名

- 身份：`AgentID` 绑定一对签名密钥（建议 Ed25519）。
- 所有事件必须由 `authored_by` 对应的私钥签名；验签失败的事件无效。
- 身份可以**代理/委托**（一个实体控制多个 `AgentID`），但协议应提供**地址聚类检测**作为反自我交易的手段（算法见 §6）。
- 身份吊销 / 密钥轮换：通过新事件 `key_rotation` 声明，并引用旧密钥签名。

---

## 5. 账本与可验证性要求

协议不指定账本技术，但要求任何合规实现满足：

1. **追加写不可变**：已确认事件不可被修改或删除；
2. **可审计**：任意第三方可重放事件流，独立重建谱系、支付与归算值；
3. **防重排**：`prev_hash` 链式 + 周期 `merkle_root` 承诺（可锚定到任意廉价公共锚点作为"公证"，非必需）；
4. **可导出**：提供标准导出接口，保证互操作（同一事件流在两个实现上结果一致）。

> 说明：一条追加写的、带哈希链与签名的日志即满足上述要求；是否上链、上哪条链，属于实现选择，不影响协议一致性。

---

## 6. 争议与反操纵

### 6.1 挑战（Challenge）

- 任何节点可对数据集发起 `challenge`，理由包括：虚假血缘、内容造假、自我交易、垃圾数据。
- 挑战需抵押 `deposit`；裁定规则（协议外，可治理）：赢者拿回抵押并获奖励，输者被扣除抵押并降信誉。

### 6.2 反自我交易

- **经济层**：协议留存 $\mu > 0$ + 不受控上游归因 ⇒ 自我引用严格亏本（见 §3 归因规则）。
- **算法层**：地址聚类检测，将控制同一批私钥的 `AgentID` 归并为同一实体；聚类实体的自我边直接标记为无效或降权。

---

## 7. 未决项 / 路线图

- [ ] $\alpha$ 权重默认规则的完整定义（均匀衰减 vs. 贡献占比）与版本化机制；
- [ ] `R_v`（外部收入）的归属规则——外部收入如何在叶子/终端数据集上登记；
- [ ] 争议仲裁的具体流程与保证金参数；
- [ ] 互操作测试向量（golden events）——用于验证两个独立实现输出一致的 $V$；
- [ ] 冷启动阶段（零价试用）的事件表达：`trial` 事件应如何记账而不产生支付边。

---

*DIN Protocol Draft v0.1 — 配套 `Spec.md`（机制与原理）。*
