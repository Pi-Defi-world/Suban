// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";

/**
 * @title PUSDToken
 * @notice ERC-20 representation of PUSD on Arc
 * @dev Minter/Burner roles restricted to ArcBridge contract
 */
contract PUSDToken is ERC20, AccessControl, Pausable {
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant BURNER_ROLE = keccak256("BURNER_ROLE");

    uint256 public immutable MAX_SUPPLY;
    address public bridge;

    event BridgeUpdated(address indexed oldBridge, address indexed newBridge);

    /**
     * @param initialSupplyCap Maximum PUSD that can exist on Arc
     * @param admin Address that receives DEFAULT_ADMIN_ROLE
     */
    constructor(uint256 initialSupplyCap, address admin) ERC20("Pi USD", "PUSD") {
        MAX_SUPPLY = initialSupplyCap;
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
    }

    modifier onlyBridge() {
        require(msg.sender == bridge, "PUSD: caller is not the bridge");
        _;
    }

    function setBridge(address _bridge) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(_bridge != address(0), "PUSD: zero address");
        address old = bridge;
        bridge = _bridge;
        emit BridgeUpdated(old, _bridge);
    }

    function mint(address to, uint256 amount) external onlyRole(MINTER_ROLE) onlyBridge {
        require(totalSupply() + amount <= MAX_SUPPLY, "PUSD: exceeds supply cap");
        _mint(to, amount);
    }

    function burn(address from, uint256 amount) external onlyRole(BURNER_ROLE) onlyBridge {
        _burn(from, amount);
    }

    function pause() external onlyRole(DEFAULT_ADMIN_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(DEFAULT_ADMIN_ROLE) {
        _unpause();
    }

    function _update(address from, address to, uint256 value) internal override whenNotPaused {
        super._update(from, to, value);
    }

    function supportsInterface(bytes4 interfaceId) public view override returns (bool) {
        return super.supportsInterface(interfaceId);
    }
}
