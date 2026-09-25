// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";
import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "./PUSDToken.sol";

/**
 * @title ArcBridge
 * @notice Validator-set bridge for PUSD cross-chain mint/burn on Arc
 * @dev M-of-N signature verification for minting, direct burn for outgoing
 */
contract ArcBridge is AccessControl, Pausable {
    using ECDSA for bytes32;

    bytes32 public constant RELAYER_ROLE = keccak256("RELAYER_ROLE");

    struct ChainState {
        uint256 totalMinted;
        uint256 totalBurned;
        uint256 mintCap;
        uint256 lastReset;
    }

    PUSDToken public pusd;

    // Validator management
    address[] public validators;
    mapping(address => bool) public isValidator;
    uint256 public threshold;

    // Per-user nonces (source chain => user => nonce)
    mapping(string => mapping(address => uint256)) public nonces;

    // Processed cross-chain tx hashes (replay protection)
    mapping(bytes32 => bool) public processedHashes;

    // Per-chain state
    mapping(string => ChainState) public chainStates;

    // Circuit breaker: max volume per window (in PUSD base units)
    uint256 public volumeCap;
    uint256 public volumeWindow;
    uint256 public windowStart;
    uint256 public windowVolume;

    // Events
    event PusdMinted(
        string indexed sourceChain,
        address indexed recipient,
        uint256 amount,
        uint256 nonce,
        bytes32 sourceTxHash
    );
    event PusdBurned(
        string indexed destChain,
        address indexed sender,
        uint256 amount,
        uint256 nonce
    );
    event ValidatorAdded(address indexed validator);
    event ValidatorRemoved(address indexed validator);
    event ThresholdUpdated(uint256 oldThreshold, uint256 newThreshold);
    event VolumeCapUpdated(uint256 oldCap, uint256 newCap);
    event CircuitBreakerTriggered(uint256 volume, uint256 cap);

    /**
     * @param _pusd Address of deployed PUSDToken
     * @param _validators Initial validator set (must be >= threshold)
     * @param _threshold M-of-N required signatures
     * @param _volumeCap Max PUSD mintable per window (0 = no limit)
     * @param _volumeWindow Window duration in seconds
     */
    constructor(
        address _pusd,
        address[] memory _validators,
        uint256 _threshold,
        uint256 _volumeCap,
        uint256 _volumeWindow
    ) {
        require(_pusd != address(0), "Bridge: zero PUSD address");
        require(_validators.length >= 2, "Bridge: need >= 2 validators");
        require(_threshold > 0 && _threshold <= _validators.length, "Bridge: invalid threshold");

        pusd = PUSDToken(_pusd);
        validators = _validators;
        threshold = _threshold;
        volumeCap = _volumeCap;
        volumeWindow = _volumeWindow;
        windowStart = block.timestamp;

        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(RELAYER_ROLE, msg.sender);

        for (uint256 i = 0; i < _validators.length; i++) {
            require(_validators[i] != address(0), "Bridge: zero validator");
            isValidator[_validators[i]] = true;
            emit ValidatorAdded(_validators[i]);
        }
    }

    // ──────────────────────────────────────────────
    // Core: Mint PUSD (called by relayer after Stellar burn)
    // ──────────────────────────────────────────────

    /**
     * @notice Mint PUSD to recipient after verifying cross-chain burn
     * @param recipient Address to receive PUSD on Arc
     * @param amount Amount of PUSD to mint (6 decimals)
     * @param sourceChain Source chain identifier (e.g., "stellar")
     * @param sourceTxHash Hash of the burn transaction on source chain
     * @param sourceNonce Nonce from the source chain event
     * @param signatures M-of-N validator signatures
     */
    function mintPusd(
        address recipient,
        uint256 amount,
        string calldata sourceChain,
        bytes32 sourceTxHash,
        uint256 sourceNonce,
        bytes[] calldata signatures
    ) external onlyRole(RELAYER_ROLE) whenNotPaused {
        require(recipient != address(0), "Bridge: zero recipient");
        require(amount > 0, "Bridge: zero amount");

        // Build hash for replay protection
        bytes32 bridgeHash = keccak256(
            abi.encodePacked(sourceChain, sourceTxHash, sourceNonce, amount, recipient)
        );
        require(!processedHashes[bridgeHash], "Bridge: already processed");

        // Verify M-of-N signatures
        require(signatures.length >= threshold, "Bridge: insufficient signatures");
        bytes32 digest = keccak256(
            abi.encodePacked("\x19Ethereum Signed Message:\n32", bridgeHash)
        );
        _verifySignatures(digest, signatures);

        // Check circuit breaker
        _checkCircuitBreaker(amount);

        // Mark as processed
        processedHashes[bridgeHash] = true;

        // Update chain state
        ChainState storage state = chainStates[sourceChain];
        state.totalMinted += amount;

        // Update window volume
        if (block.timestamp >= windowStart + volumeWindow) {
            windowStart = block.timestamp;
            windowVolume = 0;
        }
        windowVolume += amount;

        // Mint
        pusd.mint(recipient, amount);

        emit PusdMinted(sourceChain, recipient, amount, sourceNonce, sourceTxHash);
    }

    // ──────────────────────────────────────────────
    // Core: Burn PUSD (called by user to bridge to Stellar)
    // ──────────────────────────────────────────────

    /**
     * @notice Burn PUSD to bridge to another chain
     * @param destChain Destination chain identifier
     * @param amount Amount of PUSD to burn
     */
    function burnPusd(string calldata destChain, uint256 amount) external whenNotPaused {
        require(amount > 0, "Bridge: zero amount");
        require(bytes(destChain).length > 0, "Bridge: empty dest chain");

        uint256 nonce = nonces[destChain][msg.sender];

        // Burn
        pusd.burn(msg.sender, amount);

        // Update state
        ChainState storage state = chainStates[destChain];
        state.totalBurned += amount;
        nonces[destChain][msg.sender] = nonce + 1;

        emit PusdBurned(destChain, msg.sender, amount, nonce);
    }

    // ──────────────────────────────────────────────
    // Signature verification
    // ──────────────────────────────────────────────

    function _verifySignatures(bytes32 digest, bytes[] calldata signatures) internal view {
        for (uint256 i = 0; i < signatures.length; i++) {
            address signer = digest.recover(signatures[i]);
            require(isValidator[signer], "Bridge: invalid signer");
        }
    }

    // ──────────────────────────────────────────────
    // Circuit breaker
    // ──────────────────────────────────────────────

    function _checkCircuitBreaker(uint256 amount) internal {
        if (volumeCap == 0) return;

        if (block.timestamp >= windowStart + volumeWindow) {
            windowStart = block.timestamp;
            windowVolume = 0;
        }

        if (windowVolume + amount > volumeCap) {
            emit CircuitBreakerTriggered(windowVolume + amount, volumeCap);
            revert("Bridge: circuit breaker triggered");
        }
    }

    // ──────────────────────────────────────────────
    // Admin functions
    // ──────────────────────────────────────────────

    function addValidator(address validator) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(validator != address(0), "Bridge: zero address");
        require(!isValidator[validator], "Bridge: already validator");

        isValidator[validator] = true;
        validators.push(validator);
        emit ValidatorAdded(validator);
    }

    function removeValidator(address validator) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(isValidator[validator], "Bridge: not a validator");
        require(validators.length - 1 >= threshold, "Bridge: would break threshold");

        isValidator[validator] = false;
        for (uint256 i = 0; i < validators.length; i++) {
            if (validators[i] == validator) {
                validators[i] = validators[validators.length - 1];
                validators.pop();
                break;
            }
        }
        emit ValidatorRemoved(validator);
    }

    function setThreshold(uint256 _threshold) external onlyRole(DEFAULT_ADMIN_ROLE) {
        require(_threshold > 0 && _threshold <= validators.length, "Bridge: invalid threshold");
        uint256 old = threshold;
        threshold = _threshold;
        emit ThresholdUpdated(old, _threshold);
    }

    function setVolumeCap(uint256 _cap, uint256 _window) external onlyRole(DEFAULT_ADMIN_ROLE) {
        uint256 oldCap = volumeCap;
        volumeCap = _cap;
        volumeWindow = _window;
        emit VolumeCapUpdated(oldCap, _cap);
    }

    function setChainMintCap(string calldata chain, uint256 cap) external onlyRole(DEFAULT_ADMIN_ROLE) {
        chainStates[chain].mintCap = cap;
    }

    function pause() external onlyRole(DEFAULT_ADMIN_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(DEFAULT_ADMIN_ROLE) {
        _unpause();
    }

    // ──────────────────────────────────────────────
    // View functions
    // ──────────────────────────────────────────────

    function getValidators() external view returns (address[] memory) {
        return validators;
    }

    function getChainState(string calldata chain) external view returns (
        uint256 totalMinted,
        uint256 totalBurned,
        uint256 mintCap
    ) {
        ChainState storage state = chainStates[chain];
        return (state.totalMinted, state.totalBurned, state.mintCap);
    }

    function isProcessed(bytes32 bridgeHash) external view returns (bool) {
        return processedHashes[bridgeHash];
    }

    function getNonce(string calldata chain, address user) external view returns (uint256) {
        return nonces[chain][user];
    }
}
