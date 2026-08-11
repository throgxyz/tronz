# TRONZ_UPSTREAM.md

Mjolnir 需要 `tronz` 提供、但当前它还没提供的能力。

这份文件只收**上游改动**：每一条都是 Mjolnir 无法在本仓正确解决的问题，因为解决它就意味着复制
`tronz` 已经承担的职责（protobuf 解码、DTO 映射、txid 算法），而那是 `AGENTS.md` 明确禁止的。
本仓自己能修的事情不写在这里，写在 `DEVELOPMENT_HANDOFF.md`。

每条包含：现状与证据、被卡住的命令、建议改法、破坏性判断、本仓当前的处理方式。**证据里的行号
按 2026-07 的 `tronz` 0.5.0 checkout 记录，读的时候以符号名为准。**

已经落地的两次上游修复（`estimate_bandwidth` 少算 64、连接失败只报 `transport error`）记在
`DEVELOPMENT_HANDOFF.md` 第 11、12 节，不在此重复。

---

## 1. `BlockInfo` 只有三个字段

**优先级：高。** 改动最小、收益直接，而且需要的数据已经解出来了。

**现状。** `crates/rpc-types/src/types/block.rs` 的 `BlockInfo` 只有 `number`、`hash`、
`timestamp`。而 `crates/rpc-types/src/light_block.rs` 里那个为了不 materialize 交易列表而手写的
轻量视图 `BlockHeaderRawSummaryProto`，只取了 `timestamp`(tag 1) 和 `number`(tag 7)：

```rust
struct BlockHeaderRawSummaryProto {
    #[prost(int64, tag = "1")] timestamp: i64,
    #[prost(int64, tag = "7")] number: i64,
}
```

而同一条 `block_header.raw_data` 消息里还有（见 `crates/rpc-types/src/generated/protocol.rs` 的
`block_header::Raw`）：

| 字段 | tag |
| --- | --- |
| `tx_trie_root` | 2 |
| `parent_hash` | 3 |
| `witness_address` | 9 |
| `version` | 10 |

这条消息**已经在被解码**，只是这几个字段没被搬进 `BlockInfo`。代价接近于零——多解几个小 byte
vector，不触碰交易列表。

**被卡住的命令。** `mjol block <number>` / `mjol block latest` 目前只能输出三个字段。

**建议改法。** 在 `BlockHeaderRawSummaryProto` 里补上这四个 tag，并给 `BlockInfo` 加对应字段：

```rust
pub parent_hash: Option<B256>,
pub witness: Option<Address>,
pub version: i32,
pub tx_trie_root: Option<B256>,
```

`Option` 只表示**字段缺失**（genesis 没有 parent，节点也可能省略），不表示「解不出来就算了」。
字段存在但长度不对（`witness_address` 应为 21 字节、两个 hash 应为 32 字节）必须是
`ResponseError::Malformed`，与本 crate 既有约定一致：`codec::addresses_from_proto` 遇到坏地址返回
`Malformed`，`BlockSummaryProto::into_block_lookup` 把「有 block id 却没有 header」判成 `Malformed`
而不是 `None`。把损坏数据静默变成 `None` 是这个 crate 已经拒绝过的模式。

让这条安全的关键在另一处：`hash` 取自响应的 `block_id`，或调用方传入的 fallback，**从不由这个轻量
视图重算**（见 `into_block_info`）。所以往视图里加字段不可能影响 block hash。

**破坏性。** 不破坏。`BlockInfo` 是 `#[non_exhaustive]`，外部无法穷尽构造；`BlockInfo::new`
（TAPOS 用）保持签名不变，新字段取 `None` / 默认值即可。仓内只有一处结构体字面量构造
（`light_block.rs` 的 `into_block_info`），`test-utils::block` 也要同步。

**明确不要求 `transaction_count`。** 它要 tag 1 的交易列表，而这个轻量视图存在的全部意义就是跳过
它。为了一个计数把每个区块的交易都解出来是错的取舍，Mjolnir 不需要它到这个程度。

**本仓当前处理。** 技术设计 10.1 的「Block 输出字段」列出了待补字段并注明等 SDK 扩展。

---

## 2. `RawTransaction` 不公开 contract type、owner、fee_limit、memo

**优先级：高。** 这是 `mjol tx` 输出单薄的唯一原因。

**现状。** `crates/rpc-types/src/types/transaction.rs` 的 `RawTransaction` 只公开 `expiration`、
`timestamp` 和 `tx_id()`；承载其余一切的 `raw_proto` 是私有的，唯一出口是
`#[doc(hidden)]` 的 `encoded()`。也就是说交易的**合约类型、owner 地址、fee limit、memo** 都拿不到，
尽管 `apply_request_fields` 内部就在解码同一份字节并读写 `raw_data.fee_limit` / `raw_data.data`。

**被卡住的命令。** `mjol tx <hash>` 只能输出 txid、timestamp、expiration、signature_count、
byte_size。合约类型和 owner 恰恰是排查一笔交易时最先要看的两项。（`get_transaction` 返回
`Option<SignedTransaction>`，而 `SignedTransaction.raw` 就是这个 `RawTransaction`，能输出的那五项
正好是它加上 `signatures.len()` 和 `byte_size()`——所以新访问器落在 `RawTransaction` 上就够。）

**建议改法。** 一个按需调用、一次解码就取齐所有字段的视图方法——
`RawTransaction::details(&self) -> Result<TransactionDetails, ResponseError>`。是**方法**而不是
`RawTransaction` 上的即时字段：否则每次 send 都要白付一次解码，而发送路径根本不需要这些信息。
protobuf `Transaction.raw.contract` 是 repeated，SDK 不能因为节点通常只构建一个 contract 就把公开
模型写成单数：

```rust
pub struct TransactionDetails {
    pub fee_limit: Trx,
    pub memo: Bytes,
    pub contracts: Vec<TransactionContractDetails>,
}

pub struct TransactionContractDetails {
    pub kind: ContractKind,
    pub owner: Option<Address>,
}
```

这里**不能**复用现有的领域 `ContractType`，理由不是「它带完整参数、解 `Any` 麻烦」，而是它是**故意
穷尽**的。`types/contract.rs` 的类型文档写得很直白：provider 把每个变体路由到构建它的那个端点，
「that table has to stop compiling when a variant is added without a route」。那个枚举服务的是**构造
请求**方向，靠穷尽性保证漏了路由就编译不过。读取方向必须容纳节点返回的、以及将来新增的未知类型，
硬塞进去正好破坏这个不变式。所以需要一个容错的、不带 payload 的公开 `ContractKind`，未知类型保留
原始 `i32`。

`owner` 不是 `Transaction.raw.contract` 的顶层字段（那里只有 `type` / `parameter` / `provider` /
`contract_name` / `permission_id`），要从 `Any.parameter` 里取，因此允许 `None`。但**不需要**为此写
37 个解码分支：绝大多数 native contract 的 `owner_address` 都是 tag 1，只需一张 kind → owner tag 的
小表加一个单字段探测消息。这张表不能省——不能假设 owner 恒在 tag 1：

| 例外 | owner 的 tag |
| --- | --- |
| `TransferAssetContract` | 2（tag 1 是 `asset_name`） |
| `AccountUpdateContract` | 2 |
| `SetAccountIdContract` | 2 |
| `ShieldedTransferContract` | 无 owner |

盲目按 tag 1 解会把 TRC10 转账的资产名当成地址读出来，这正是必须由 SDK 而不是调用方来做的那类
细节。

如果以后需要完整参数，再单独提供 `decode_contract() -> Result<ContractType, ResponseError>`。注意这
是全新工作而不是重构：`ContractType` 覆盖了 41 个 proto contract type 中的 37 个，但目前**没有任何
proto → `ContractType` 的解码路径**（没有 `TryFrom`，它只被用来构造请求）。这也是分两步走的依据。

**破坏性。** 加方法或加新类型都不破坏现有 API。

**本仓当前处理。** 技术设计 10.1「Transaction 输出字段」注明了这批字段未公开。Mjolnir **不会**
自己 prost 解码来绕过——那正是禁止复制的那类逻辑。

---

## 3. `txid` 依赖 `#[doc(hidden)]` 的入口

**优先级：中。** 现在能用，但随时可能在不升主版本的情况下断掉。

**现状。** `RawTransaction::from_node_encoded(encoded, claimed_tx_id)` 标了 `#[doc(hidden)]`，
文档写明「公开是因为 transport crate 要构造，隐藏是因为它们是唯一调用方」。而
`mjol txid <transaction>` 正是靠它把一份编码后的 `Transaction` 算成 txid。

**被卡住的命令。** `mjol txid`（已实现，但踩在隐藏 API 上）。

**建议改法。** 提供一个正式的公开入口，二者其一即可：

1. `RawTransaction::decode(bytes) -> Result<Self, ResponseError>`，语义就是「解析一份节点编码的
   交易」，不带 `claimed_tx_id` 这个只有 transport 才需要的参数；
2. 或者在 `tronz-rpc-types` 里放一个
   `transaction_id(encoded_transaction) -> Result<TxId, ResponseError>`。

不要放进 `tronz-primitives`：计算需要理解 protobuf `Transaction` / `raw_data`，会让 primitives
反向依赖 prost 和 RPC schema。入口还应明确输入是**完整编码后的 `Transaction`**；txid 的算法是
`sha256(protobuf_encode(Transaction.raw_data))`，不是直接 hash 整个输入。

顺带一个 Mjolnir 无法自查的正确性问题：误把里层的 `raw_data` 当成完整 `Transaction` 传进去时，
目前只靠「`raw_data` 的首字段不是长度分隔的嵌套消息，protobuf 解码极可能失败」来兜。这个校验应该
由 `tronz` 做，具体是：入口要求 `raw_data` 存在**且 `contract` 非空**——`from_node_encoded` 现在只
要求前者。误传的字节几乎不可能凑出一个非空且类型合法的 contract 列表，所以这能把误用从「静默算出
一个错 txid」变成「报错」。

但要说清楚：两种形状在 protobuf 线格式上本来就兼容（都是 length-delimited），这只是让误用在现实中
几乎必然失败的启发式，**不是**能证明输入形状的根治手段。真正的保证只能来自调用方知道自己传的是
什么。

**破坏性。** 加公开入口不破坏；`from_node_encoded` 可以继续保留给 transport 用。

**本仓当前处理。** 技术设计 10.3 和 handoff 4.2 都写明了这是**有意接受**的依赖，并用
`tests/fixtures/` 里一笔真实主网交易加节点报告的 txid 锁住语义，升级后一旦漂移测试立刻失败。

---

## 4. `TransactionInfo` 缺内部交易和几个计费字段

**优先级：中。**

**现状。** `crates/rpc-types/src/types/receipt.rs` 的 `TransactionInfo` 有 `logs`，但**没有内部
交易**。计费侧的字段被扁平化成了 `energy_usage` / `energy_fee` / `net_usage` / `net_fee` 四个，其中
`energy_usage` 实际由 codec 映射自 java-tron 的 `receipt.energy_usage_total`（`codec/mod.rs`），所以
total 并没有丢，只是公开名称不同。

值得注意的是同一个文件里**已经有**一个公开的领域 `ResourceReceipt`，六个字段齐全，
`origin_energy_usage` 和 `energy_usage_total` 都在，也从 `types/mod.rs` 导出了——但全仓没有任何一处
构造它（codec 里出现的都是 `proto::ResourceReceipt`）。它是一个死掉的公开类型。所以真正缺的是：
把它接上，加上顶层总 `fee`（proto tag 2）和内部交易。

**被卡住的命令。** `mjol receipt` 拿不到内部调用，也就无法解释一笔合约交易到底 touch 了什么；
`energy_usage` 已对应 tronscan 使用的 total，但缺少 origin 与总 fee 时用户仍无法完整对账。

**建议改法。** 给 `TransactionInfo` 加 `receipt: ResourceReceipt` 并真的填充它，而不是把
`origin_energy_usage` 单独扁平化上去——后者会让同一个语义在公开 API 里出现两次。这一改同时解决了
「需要时区分 receipt 原始 `energy_usage` 与 total」：两个值本来就在 `ResourceReceipt` 里各占一个
字段，现有扁平的 `TransactionInfo.energy_usage` 原样保留，不改名、不新增同义字段。顺带把 proto
`ResourceReceipt` 有而领域类型没有的 `energy_penalty_total`（tag 8）一起补上，或明确写不要。

再加顶层 `fee`，以及 `internal_transactions`。后者需要一个 DTO，字段要照 java-tron 的
`InternalTransaction`（proto tag 17，已生成）来定，注意两处别写丢：`call_value_info` 是 repeated 的
`(call_value, token_id)`，不是单个金额；还有一个 `rejected` 标志，区分「被回滚的内部调用」。

**破坏性。** 不破坏，`TransactionInfo` 是 `#[non_exhaustive]`。注意它同时需要
`tronz-rpc-types` 的 `test-utils` 构造器同步更新（`transaction_info`），否则下游测试构造不出新字段。

**本仓当前处理。** 技术设计 10.1「Receipt 输出字段」注明完整 logs 与 internal transactions 待
SDK 提供；当前 `receipt` 输出 `log_count`，`mjol logs` 单独解事件。

---

## 5. `test-utils` 的构造器覆盖不全

**优先级：低。**

**现状。** 节点响应类型基本都是 `#[non_exhaustive]`，下游只能通过 `tronz-rpc-types` 的
`test-utils` feature 构造。目前约 21 个 non-exhaustive struct 只有 6 个 helper，属于少数常用类型
已有构造器（`block`、`transaction_info` 等），不是多数。像 `DelegatedResourceIndex` 这样既
`#[non_exhaustive]` 又不 derive `Default` 的类型，下游只能绕开。

**被卡住的东西。** 不卡命令，卡的是测试覆盖：缺构造器的类型只能测纯格式函数、错误传播和 CLI 本地
预校验，测不到「节点这样答、命令就该那样输出」。

**建议改法。** 对 provider/mock 测试实际需要构造的节点响应类型同步提供 builder；不要求为所有 DTO
预先铺满 `test-utils`。具体缺哪些，等真正被卡住时再列。

**本仓当前处理。** handoff 第 6 节写明了这个限制和替代做法。

---

## 6. 没有「等 N 个确认」的入口

**优先级：低，可能永远不需要。**

**现状。** `PendingTransaction` 只有两档：轮询到被节点索引（`get_receipt`）和等待固化
（`get_solidified_receipt`）。没有公开的「等 N 个区块确认」。

**被卡住的命令。** `mjol send --confirmations N`（Cast 有这个参数）。

**建议改法。** 先不提。TRON 有明确的固化（不可逆）语义，`--solidified` 已经覆盖了「我要确定它不会
回滚」这个真实需求；「等 N 个确认」是从 Ethereum 借来的近似说法，在有固化概念的链上价值有限。
**列在这里是为了说明它不做的原因，不是需求。**

**本仓当前处理。** 技术设计 11.2 与 handoff 4.3 都写明不实现，且不在 Mjolnir 里自行数区块。

---

## 附：核对后确认**不需要**上游改动的项

写这份文件时顺带核对了几条本仓文档里记成「等上游」的条目，其中有记错的：

- **广播被节点拒绝可以区分，不需要新增变体。** handoff 第 7 节曾写「`tronz` 把广播失败报成
  `ProviderError::Transport`，无法在不解析错误文本的前提下与网络故障区分」，因此退出码 `5`
  未启用。实际上 `ProviderError`（即 `RpcError<TransportErrorKind>`）有独立的
  `NodeError(String)` 变体，对应节点的 `Return { result: false }`，靠匹配变体就能区分，不必碰
  错误文本。另有 `RpcError::Broadcast { tx_id, source }` 表示「已经广播出去但没等到确认，状态未
  知」，并带上 txid——这对脚本是一条重要且当前被 Mjolnir 归并掉的状态。**这是本仓的待办，不是上游
  的**：启用退出码 5，并单独映射 `Broadcast`。
- **固化回执不需要上游支持。** `tronz` 的 `mock` feature 已经提供 `MockSolidityTransport`，
  `SolidityProvider::new` 收 `impl SolidityTransport`，所以 `receipt --solidified` 连测试都能在
  本仓闭环。已实现，见技术设计 10.1.1。
- **连接超时与重试不需要新旋钮。** `ProviderBuilder` 自带 connect / request 超时和重试策略，
  Mjolnir 只是没暴露对应参数，属于本仓的取舍。

`tvm-rs` 相关的阻塞项（不导出 inspector、状态端口只吃冻结状态、path 依赖 `../revm`、还停在
`tronz-primitives` 0.4.0）都属于 `tvm-rs` 自己，不是 `tronz` 的问题，记在技术设计 20.3。
