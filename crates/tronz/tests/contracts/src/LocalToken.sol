// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract LocalToken {
    string public constant name = "Local Token";
    string public constant symbol = "LOCAL";
    uint8 public constant decimals = 6;
    uint256 public constant totalSupply = 1_000_000_000_000;

    mapping(address => uint256) public balanceOf;

    event Transfer(address indexed from, address indexed to, uint256 value);

    constructor() {
        balanceOf[msg.sender] = totalSupply;
        emit Transfer(address(0), msg.sender, totalSupply);
    }

    function transfer(address to, uint256 value) external returns (bool) {
        require(balanceOf[msg.sender] >= value, "insufficient balance");
        balanceOf[msg.sender] -= value;
        balanceOf[to] += value;
        emit Transfer(msg.sender, to, value);
        return true;
    }
}
