//! TRC20 ABI bindings and (optionally) the provider-bound [`Trc20Instance`].
//!
//! TRC20 is byte-for-byte compatible with the EVM ERC20 ABI, so the interface
//! and all call/return codecs are generated directly by `alloy`'s
//! [`sol!`](alloy_sol_macro::sol) macro. No bespoke ABI codec is needed.

#[cfg(feature = "provider")]
pub mod instance;

use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
#[cfg(feature = "provider")]
pub use instance::{Trc20Error, Trc20Ext, Trc20Instance};
use tronz_primitives::{Address, Bytes, U256};

sol! {
    #[derive(Debug, PartialEq, Eq)]
    interface ITRC20 {
        function name()                                                  external view returns (string);
        function symbol()                                                external view returns (string);
        function decimals()                                              external view returns (uint8);
        function totalSupply()                                           external view returns (uint256);
        function balanceOf(address account)                              external view returns (uint256);
        function transfer(address to, uint256 amount)                    external returns (bool);
        function approve(address spender, uint256 amount)                external returns (bool);
        function allowance(address owner, address spender)               external view returns (uint256);
        function transferFrom(address from, address to, uint256 amount)  external returns (bool);

        event Transfer(address indexed from, address indexed to, uint256 value);
        event Approval(address indexed owner, address indexed spender, uint256 value);
    }
}

/// ABI-encode the `transfer(to, amount)` call into the calldata bytes used by a
/// `TriggerSmartContract`.
pub fn encode_transfer(to: Address, amount: U256) -> Bytes {
    ITRC20::transferCall { to: to.into(), amount }.abi_encode().into()
}

/// ABI-encode the `approve(spender, amount)` call.
pub fn encode_approve(spender: Address, amount: U256) -> Bytes {
    ITRC20::approveCall { spender: spender.into(), amount }.abi_encode().into()
}

/// ABI-encode the `transferFrom(from, to, amount)` call.
pub fn encode_transfer_from(from: Address, to: Address, amount: U256) -> Bytes {
    ITRC20::transferFromCall { from: from.into(), to: to.into(), amount }.abi_encode().into()
}

/// ABI-encode the `balanceOf(account)` constant call.
pub fn encode_balance_of(account: Address) -> Bytes {
    ITRC20::balanceOfCall { account: account.into() }.abi_encode().into()
}

/// ABI-encode the `allowance(owner, spender)` constant call.
pub fn encode_allowance(owner: Address, spender: Address) -> Bytes {
    ITRC20::allowanceCall { owner: owner.into(), spender: spender.into() }.abi_encode().into()
}

/// Decode the `uint256` returned by `balanceOf` / `allowance` / `totalSupply`.
pub fn decode_uint256_return(output: &[u8]) -> Result<U256, alloy_sol_types::Error> {
    ITRC20::balanceOfCall::abi_decode_returns(output)
}

/// Decode the `uint8` returned by `decimals`.
pub fn decode_decimals_return(output: &[u8]) -> Result<u8, alloy_sol_types::Error> {
    ITRC20::decimalsCall::abi_decode_returns(output)
}

/// Decode the `string` returned by `name` / `symbol`.
pub fn decode_string_return(output: &[u8]) -> Result<String, alloy_sol_types::Error> {
    ITRC20::nameCall::abi_decode_returns(output)
}

/// The four-byte selector for a generated call type.
pub fn selector<C: SolCall>() -> [u8; 4] {
    C::SELECTOR
}

#[cfg(test)]
mod tests {
    use alloy_sol_types::SolValue;

    use super::*;

    const ADDR: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

    fn addr() -> Address {
        ADDR.parse().unwrap()
    }

    #[test]
    fn transfer_selector_matches_erc20() {
        // keccak256("transfer(address,uint256)")[..4] == 0xa9059cbb
        let data = encode_transfer(addr(), U256::from(1u64));
        assert_eq!(&data[..4], &[0xa9, 0x05, 0x9c, 0xbb]);
        assert_eq!(ITRC20::transferCall::SELECTOR, [0xa9, 0x05, 0x9c, 0xbb]);
    }

    #[test]
    fn balance_of_selector() {
        // keccak256("balanceOf(address)")[..4] == 0x70a08231
        let data = encode_balance_of(addr());
        assert_eq!(&data[..4], &[0x70, 0xa0, 0x82, 0x31]);
    }

    #[test]
    fn transfer_encodes_evm_address_not_tron() {
        let data = encode_transfer(addr(), U256::from(0u64));
        // The address argument occupies bytes [4..36]; the low 20 bytes must be
        // the EVM body (0x41 prefix stripped), left-padded to 32 bytes.
        let arg = &data[4..36];
        assert_eq!(&arg[..12], &[0u8; 12], "address must be left-padded");
        assert_eq!(&arg[12..], addr().as_evm_bytes());
    }

    #[test]
    fn uint256_return_roundtrip() {
        let value = U256::from(123_456_789u64);
        let encoded = value.abi_encode();
        assert_eq!(decode_uint256_return(&encoded).unwrap(), value);
    }

    #[test]
    fn string_return_roundtrip() {
        let encoded = "Tether USD".to_string().abi_encode();
        assert_eq!(decode_string_return(&encoded).unwrap(), "Tether USD");
    }

    #[test]
    fn decimals_return_roundtrip() {
        // abi-encoded uint8 is a left-padded 32-byte word.
        let mut encoded = [0u8; 32];
        encoded[31] = 6;
        assert_eq!(decode_decimals_return(&encoded).unwrap(), 6u8);
    }
}
