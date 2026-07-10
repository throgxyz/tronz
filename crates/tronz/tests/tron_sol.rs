//! End-to-end: `tron_sol!` through the `tronz` umbrella crate (default path).

#![cfg(feature = "contract")]

use tronz::{
    Address, TronProvider, U256,
    contract::{SolCall as _, tron_sol},
};

tron_sol! {
    #[sol(rpc)]
    interface IERC20 {
        function decimals() external view returns (uint8);
        function balanceOf(address owner) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
    }
}

#[test]
fn default_path_encodes() {
    let owner: Address = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".parse().unwrap();
    let call = IERC20::balanceOfCall { owner: owner.into() };
    // 4-byte selector + 32-byte address argument
    assert_eq!(call.abi_encode().len(), 36);
}

#[allow(dead_code, clippy::unused_async)]
async fn _typecheck<P: TronProvider + Clone>(provider: P, addr: Address) {
    let token = IERC20::new(addr, provider);
    let _: u8 = token.decimals().call().await.unwrap();
    let _: U256 = token.balanceOf(addr).call().await.unwrap();
    let _ = token.transfer(addr, U256::ZERO).send().await;
}
