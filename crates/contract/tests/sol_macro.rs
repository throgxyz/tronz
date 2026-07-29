//! Tests for the [`tron_sol!`] macro.

#![cfg(feature = "provider")]
#![allow(
    dead_code,
    clippy::unused_async,
    reason = "generated APIs are compile-tested without invoking every helper"
)]

use tronz_contract::{SolCall as _, tron_sol};
use tronz_primitives::{Address, Bytes, U256};
use tronz_provider::{ContractReadProvider, TronProvider};

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IERC20 {
        function name() external view returns (string);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
        function balanceOf(address owner) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);
        event Transfer(address indexed from, address indexed to, uint256 value);
    }
}

#[test]
fn type_layer_encoding() {
    let owner: Address = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse().unwrap();
    let call = IERC20::balanceOfCall { owner: owner.into() };
    let encoded = call.abi_encode();
    assert_eq!(&encoded[..4], &IERC20::balanceOfCall::SELECTOR);
    assert_eq!(encoded.len(), 36);
}

#[test]
fn return_type_decode() {
    let mut out = [0u8; 32];
    out[31] = 42;
    assert_eq!(IERC20::balanceOfCall::abi_decode_returns(&out).unwrap(), U256::from(42u64),);
}

async fn _erc20_api<P: TronProvider + Clone>(provider: P, addr: Address) {
    let token = IERC20::new(addr, provider);
    let _: String = token.name().call().await.unwrap();
    let _: u8 = token.decimals().call().await.unwrap();
    let _: U256 = token.totalSupply().call().await.unwrap();
    let _: U256 = token.balanceOf(addr).call().await.unwrap();
    let _: U256 = token.allowance(addr, addr).call().await.unwrap();
    let _ = token.transfer(addr, U256::ZERO).send().await;
    let _ = token.approve(addr, U256::ZERO).send().await;
    let _ = token.transferFrom(addr, addr, U256::ZERO).send().await;
    let _ = token.balanceOf(addr).estimate_energy().await;
    let _ = token.address();
    let _ = token.clone().at(addr);
    // generic entry point
    let _ = token.call_builder(&IERC20::balanceOfCall { owner: addr.into() }).call().await;
}

async fn _erc20_read_api<P: ContractReadProvider>(provider: P, addr: Address) {
    let token = IERC20::new(addr, provider).caller(addr);
    let _: U256 = token.balanceOf(addr).call().await.unwrap();
    let _ = token.balanceOf(addr).caller(addr).estimate_energy().await;
    let _ = token.Transfer_filter().query_block(1).await;
}

fn _instance_is_debug<P: TronProvider>() {
    fn assert_debug<T: core::fmt::Debug>() {}
    assert_debug::<IERC20::Instance<P>>();
}

// overloads

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IERC721 is IBase {
        function ownerOf(uint256 tokenId) external view returns (address);
        function safeTransferFrom(address from, address to, uint256 tokenId) external;
        function safeTransferFrom(address from, address to, uint256 tokenId, bytes data) external;
    }
}

#[test]
fn overloaded_selectors_differ() {
    assert_ne!(
        IERC721::safeTransferFrom_0Call::SELECTOR,
        IERC721::safeTransferFrom_1Call::SELECTOR,
    );
}

async fn _erc721_api<P: TronProvider + Clone>(provider: P, addr: Address) {
    let nft = IERC721::new(addr, provider);
    let _ = nft.ownerOf(U256::ZERO).call().await;
    let _ = nft.safeTransferFrom_0(addr, addr, U256::ZERO).send().await;
    let _ = nft.safeTransferFrom_1(addr, addr, U256::ZERO, Bytes::new()).send().await;
}

// user-defined value types (UDVT)

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IPool {
        type Shares is uint256;
        function deposit(Shares amount) external returns (Shares);
    }
}

// Regression: UDVT parameters must map to the underlying type (U256), not the
// wrapper name, so the generated method signature matches the `…Call` struct field.
async fn _udvt_maps_to_underlying<P: TronProvider + Clone>(provider: P, addr: Address) {
    let pool = IPool::new(addr, provider);
    let _ = pool.deposit(U256::ZERO).send().await;
}

// parameters named after Rust keywords

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IKeywordParams {
        function configure(uint256 ref, address move, bool box) external;
    }
}

async fn _keyword_params<P: TronProvider + Clone>(provider: P, addr: Address) {
    let c = IKeywordParams::new(addr, provider);
    let _ = c.configure(U256::ZERO, addr, true).send().await;
}

// bytecode + deploy

tron_sol! {
    #[sol(rpc, bytecode = "0x6080604052", deployed_bytecode = "0x6080604051")]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    contract SimpleToken {
        constructor(uint256 supply);
        function totalSupply() external view returns (uint256);
    }
}

#[test]
fn bytecode_is_embedded() {
    assert_eq!(&SimpleToken::BYTECODE[..], &[0x60, 0x80, 0x60, 0x40, 0x52]);
}

#[test]
fn deployed_bytecode_is_embedded() {
    assert_eq!(&SimpleToken::DEPLOYED_BYTECODE[..], &[0x60, 0x80, 0x60, 0x40, 0x51]);
}

async fn _deploy_api<P: TronProvider + Clone>(provider: P) {
    // deploy_builder lets you chain .abi(...).name(...) before sending
    let _ = SimpleToken::deploy_builder(provider.clone(), U256::from(1000u64));
    // deploy is one-shot: returns Result<Instance<P>>
    let _ = SimpleToken::deploy(provider, U256::from(1000u64)).await;
}

// Solidity functions colliding with reserved `Instance` method names

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IReserved {
        function address() external view returns (address);
        function at(uint256 slot) external view returns (bytes32);
        function provider() external view returns (address);
    }
}

#[test]
fn reserved_names_get_call_suffix() {
    let _ = IReserved::addressCall {};
    let _ = IReserved::providerCall {};
}

async fn _reserved_api<P: TronProvider + Clone>(provider: P, addr: Address) {
    let c = IReserved::new(addr, provider);
    let _: Address = c.address();
    let _ = c.clone().at(addr);
    let _: &P = c.provider();
    let _ = c.address_call().call().await;
    let _ = c.at_call(U256::ZERO).call().await;
    let _ = c.provider_call().call().await;
}

// multiple items in a single invocation: several contracts mixed with bare
// `struct`/`enum` definitions (previously each needed its own `tron_sol!` block)

tron_sol! {
    #[tron_sol(tronz_crate = ::tronz_contract)]

    struct Order {
        address maker;
        uint256 amount;
    }

    enum Side {
        Buy,
        Sell,
    }

    #[sol(rpc)]
    interface IExchange {
        function place(Order order, Side side) external returns (uint256 id);
    }

    #[sol(rpc)]
    interface IRegistry {
        function lookup(uint256 id) external view returns (address);
    }
}

#[test]
fn multiple_items_one_invocation() {
    // bare struct/enum from the same block are usable as types
    let owner: Address = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse().unwrap();
    let _order = Order { maker: owner.into(), amount: U256::from(7u64) };
    let _side = Side::Buy;
    // both interfaces produced their own selectors + instances
    let _ = IExchange::placeCall::SELECTOR;
    let _ = IRegistry::lookupCall::SELECTOR;
}

async fn _multi_instance_api<P: TronProvider + Clone>(provider: P, addr: Address) {
    let exchange = IExchange::new(addr, provider.clone());
    let _ =
        exchange.place(Order { maker: addr.into(), amount: U256::ZERO }, Side::Sell).call().await;
    let registry = IRegistry::new(addr, provider);
    let _ = registry.lookup(U256::ZERO).call().await;
}

// attribute passthrough: top-level `#[derive(...)]` and `#[sol(extra_derives(...))]`
// reach the generated type layer (previously dropped)

tron_sol! {
    #[tron_sol(tronz_crate = ::tronz_contract)]

    #[derive(Debug, Default, PartialEq)]
    struct Point {
        int256 x;
        int256 y;
    }

    #[derive(Debug)]
    #[sol(rpc)]
    interface IPoints {
        function set(Point p) external;
    }
}

#[test]
fn attributes_pass_through() {
    // `#[derive(Debug, Default, PartialEq)]` was forwarded to the type layer
    let a = Point::default();
    let b = Point { x: Default::default(), y: Default::default() };
    assert_eq!(a, b);
    // `Debug` is available because the derive was forwarded
    let _ = format!("{a:?}");
    // the call struct also derives Debug (from the `#[derive(Debug)]` on the interface)
    let _ = format!("{:?}", IPoints::setCall { p: Point::default() });
}

// public state variables → getter methods

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    contract IVault {
        uint256 public totalDeposits;
        address public owner;
        function deposit(uint256 amount) external;
    }
}

#[test]
fn public_state_var_selectors_exist() {
    // alloy's sol! generates xxxCall structs for public state variables;
    // our Instance must expose matching typed getter methods.
    let _ = IVault::totalDepositsCall::SELECTOR;
    let _ = IVault::ownerCall::SELECTOR;
}

async fn _vault_getters<P: TronProvider + Clone>(provider: P, addr: Address) {
    let vault = IVault::new(addr, provider);
    let _: U256 = vault.totalDeposits().call().await.unwrap();
    let _ = vault.owner().call().await.unwrap(); // returns alloy_primitives::Address
}

// event filter methods

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IToken {
        event Transfer(address indexed from, address indexed to, uint256 value);
        event Approval(address indexed owner, address indexed spender, uint256 value);
        function transfer(address to, uint256 amount) external returns (bool);
    }
}

async fn _event_filter_api<P: TronProvider + Clone>(provider: P, addr: Address) {
    use tronz_primitives::B256;
    let token = IToken::new(addr, provider);
    // generic entry point
    let _filter = token.event_filter::<IToken::Transfer>();
    // per-event convenience methods
    let _tf = token.Transfer_filter();
    let _af = token.Approval_filter();
    // filter with address override
    let _tf2 = token.Transfer_filter().address(addr);
    // query by tx id
    let tx_id = B256::ZERO;
    let _: Vec<IToken::Transfer> = _tf.query_tx(tx_id).await.unwrap();
    // query by block
    let _: Vec<IToken::Transfer> = token.Transfer_filter().query_block(0).await.unwrap();
}

// ── JSON ABI file path ────────────────────────────────────────────────────────

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    IERC20Json, "tests/abi/erc20.json"
}

#[test]
fn json_abi_type_layer_encoding() {
    let owner: Address = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse().unwrap();
    let call = IERC20Json::balanceOfCall { owner: owner.into() };
    let encoded = call.abi_encode();
    assert_eq!(&encoded[..4], &IERC20Json::balanceOfCall::SELECTOR);
    assert_eq!(encoded.len(), 36);
}

async fn _json_abi_rpc_api<P: TronProvider + Clone>(provider: P, addr: Address) {
    let token = IERC20Json::new(addr, provider);
    let _: String = token.name().call().await.unwrap();
    let _: u8 = token.decimals().call().await.unwrap();
    let _: U256 = token.totalSupply().call().await.unwrap();
    let _: U256 = token.balanceOf(addr).call().await.unwrap();
    let _ = token.transfer(addr, U256::ZERO).send().await;
}

// Forge artifact format

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    IForgeArtifact, "tests/abi/erc20_forge.json"
}

#[test]
fn forge_artifact_matches_raw_array() {
    // Forge `{"abi":[...]}` and raw `[...]` must produce identical signatures.
    assert_eq!(IForgeArtifact::totalSupplyCall::SIGNATURE, IERC20Json::totalSupplyCall::SIGNATURE);
    assert_eq!(IForgeArtifact::balanceOfCall::SIGNATURE, IERC20Json::balanceOfCall::SIGNATURE);
}

// inner attributes

tron_sol! {
    #![sol(all_derives)]
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    IInnerAttr, "tests/abi/erc20.json"
}

#[test]
fn inner_attr_all_derives() {
    let owner: Address = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse().unwrap();
    let call = IInnerAttr::balanceOfCall { owner: owner.into() };
    let _ = format!("{call:?}");
    let _ = call.clone();
}

// ── sol! parity: parameter shapes and type mapping ────────────────────────────

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IUnnamed {
        function one(uint256) external;
        function two(uint256, string) external;
        function none() external;
        function oneNamed(uint256 x) external;
    }
}

#[test]
fn single_unnamed_param_is_a_tuple_struct() {
    // `sol!` deconstructs exactly this shape into `oneCall(pub U256)`; every
    // other arity keeps named `_0`/`_1` fields.
    let one = IUnnamed::oneCall(U256::ZERO);
    assert_eq!(one.abi_encode().len(), 36);
    let two = IUnnamed::twoCall { _0: U256::ZERO, _1: String::new() };
    assert_eq!(&two.abi_encode()[..4], &IUnnamed::twoCall::SELECTOR);
}

async fn _unnamed_api<P: TronProvider + Clone>(provider: P, addr: Address) {
    let c = IUnnamed::new(addr, provider);
    let _ = c.one(U256::ZERO).send().await;
    let _ = c.two(U256::ZERO, String::new()).send().await;
    let _ = c.none().send().await;
    let _ = c.oneNamed(U256::ZERO).send().await;
}

tron_sol! {
    library L {
        struct S { uint256 x; }
    }

    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IQualified {
        function f(L.S memory s) external;
    }
}

async fn _qualified_custom_type<P: TronProvider + Clone>(provider: P, addr: Address) {
    // The custom path must stay whole: `L.S` resolves to `L::S`, not `S`.
    let _ = IQualified::new(addr, provider).f(L::S { x: U256::ZERO }).send().await;
}

tron_sol! {
    interface IHeld {
        function decimals() external view returns (uint8);
    }

    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IHolderVault {
        function setToken(IHeld token) external;
        function setTokens(IHeld[] memory tokens) external;
    }
}

async fn _contract_typed_params_are_addresses<P: TronProvider + Clone>(provider: P, addr: Address) {
    // A contract-typed parameter is an `address` on the wire.
    let v = IHolderVault::new(addr, provider);
    let _ = v.setToken(addr).send().await;
    let _ = v.setTokens(vec![addr.into()]).send().await;
}

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface ICallback {
        function register(function() external cb) external;
    }
}

async fn _function_type_param<P: TronProvider + Clone>(provider: P, addr: Address) {
    let _ = ICallback::new(addr, provider).register(alloy_primitives::Function::ZERO).send().await;
}

// ── inheritance ───────────────────────────────────────────────────────────────

tron_sol! {
    interface IBaseDefined {
        function baseFn() external view returns (uint256);
    }

    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IDerived is IBaseDefined {
        function ownFn() external view returns (uint256);
    }
}

#[test]
fn inherited_members_stay_in_the_base_module() {
    // Like `sol!`, inheritance is not flattened: `IDerived` gets no `baseFnCall`
    // and no `baseFn` instance method. Callers reach base members through the
    // base interface.
    let _ = IBaseDefined::baseFnCall {};
    assert_eq!(IDerived::ownFnCall::SIGNATURE, "ownFn()");
}

async fn _derived_api<P: TronProvider + Clone>(provider: P, addr: Address) {
    let _: U256 = IDerived::new(addr, provider).ownFn().call().await.unwrap();
}

// ── JSON ABI: outer alloy attributes ──────────────────────────────────────────

tron_sol! {
    #[derive(Debug)]
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    IOuterAttr, "tests/abi/erc20.json"
}

#[test]
fn json_abi_keeps_outer_attributes() {
    // Only the TRON-specific attributes are consumed; `#[derive(…)]` reaches `sol!`.
    let _ = format!("{:?}", IOuterAttr::decimalsCall {});
}

// ── reserved instance methods ─────────────────────────────────────────────────

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    interface IShadow {
        function caller() external view returns (address);
        function set_caller(address who) external;
    }
}

async fn _caller_accessors_are_not_shadowed<P: TronProvider + Clone>(provider: P, addr: Address) {
    // The instance's own `caller`/`set_caller` win; the Solidity functions of
    // the same name are reached through the `_call` suffix.
    let mut c = IShadow::new(addr, provider).caller(addr);
    c.set_caller(addr);
    let _ = c.caller_call().call().await;
    let _ = c.set_caller_call(addr).send().await;
}

// ── bytecode without `#[sol(rpc)]` ────────────────────────────────────────────

tron_sol! {
    #[sol(bytecode = "0x60806040")]
    #[sol(deployed_bytecode = "0x0102")]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    contract IPlain {
        function ping() external;
    }
}

#[test]
fn bytecode_constants_do_not_need_rpc() {
    // `sol!` emits these regardless of `rpc`; only the deploy helpers need it.
    assert!(!IPlain::BYTECODE.is_empty());
    assert!(!IPlain::DEPLOYED_BYTECODE.is_empty());
}

// ── a contract's own types vs a top-level contract of the same name ───────────

tron_sol! {
    contract Coin {}

    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    contract CoinVault {
        struct Coin { uint256 id; }

        function store(Coin value) external;
        function storeMany(Coin[] memory values) external;
    }
}

#[test]
fn a_top_level_contract_wins_over_a_same_named_local_type() {
    // `sol!` resolves `Coin` to the top-level contract, so the field is an
    // address rather than the local struct. The instance layer must agree.
    let _: alloy_primitives::Address = CoinVault::storeCall { value: Address::ZERO.into() }.value;
    // The shadowed local struct stays reachable by name.
    let _ = CoinVault::Coin { id: U256::ZERO };
}

async fn _shadowed_params_are_addresses<P: TronProvider + Clone>(provider: P, addr: Address) {
    let v = CoinVault::new(addr, provider);
    let _ = v.store(addr).send().await;
    let _ = v.storeMany(vec![addr.into()]).send().await;
}

// ── Forge artifact bytecode ───────────────────────────────────────────────────

tron_sol! {
    #[sol(rpc)]
    #[tron_sol(tronz_crate = ::tronz_contract)]
    IArtifact, "tests/abi/erc20_forge.json"
}

#[test]
fn forge_artifact_bytecode_is_picked_up() {
    // The artifact's `bytecode`/`deployedBytecode` reach the type layer without
    // being restated as attributes, just as they do for `sol!`.
    assert!(!IArtifact::BYTECODE.is_empty());
    assert!(!IArtifact::DEPLOYED_BYTECODE.is_empty());
}

async fn _deploy_from_artifact<P: TronProvider + Clone>(provider: P) {
    let _ = IArtifact::deploy(provider).await;
}
