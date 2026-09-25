import * as dotenv from "dotenv";
dotenv.config();

export const config = {
  // Stellar/Pi configuration
  stellar: {
    rpcUrl: process.env.STELLAR_RPC_URL || "https://rpc.suban.org",
    horizonUrl: process.env.STELLAR_HORIZON_URL || "https://horizon.suban.org",
    networkPassphrase: process.env.STELLAR_NETWORK_PASSPHRASE || "Pi Network",
    bridgeContract: process.env.STELLAR_BRIDGE_CONTRACT || "",
    pusdToken: process.env.STELLAR_PUSD_TOKEN || "",
  },

  // Arc configuration
  arc: {
    rpcUrl: process.env.ARC_RPC_URL || "https://rpc.testnet.arc.io",
    chainId: parseInt(process.env.ARC_CHAIN_ID || "5042002"),
    bridgeContract: process.env.ARC_BRIDGE_CONTRACT || "",
    pusdToken: process.env.ARC_PUSD_TOKEN || "",
  },

  // Relayer keys
  keys: {
    // Stellar signer keypair (for signing cross-chain proofs)
    stellarSigner: process.env.STELLAR_SIGNER_SECRET || "",
    // EVM signer private key (for signing Arc transactions)
    evmSigner: process.env.EVM_SIGNER_PRIVATE_KEY || "",
  },

  // Fee configuration
  fees: {
    // Bridge fee percentage (e.g., 0.5 = 0.5%)
    percentage: parseFloat(process.env.BRIDGE_FEE_PERCENTAGE || "0.5"),
    // Minimum fee in PUSD
    minimumFee: parseFloat(process.env.BRIDGE_MINIMUM_FEE || "1"),
    // Protocol share (e.g., 0.1 = 10% of fee goes to protocol)
    protocolShare: parseFloat(process.env.BRIDGE_PROTOCOL_SHARE || "0.1"),
  },

  // Circuit breaker
  circuitBreaker: {
    // Max volume per hour in PUSD
    maxVolumePerHour: parseFloat(process.env.CIRCUIT_BREAKER_MAX_VOLUME || "1000000"),
    // Auto-pause on anomaly
    autoPause: process.env.CIRCUIT_BREAKER_AUTO_PAUSE === "true",
  },

  // Polling intervals (ms)
  polling: {
    stellar: parseInt(process.env.STELLAR_POLL_INTERVAL || "5000"),
    arc: parseInt(process.env.ARC_POLL_INTERVAL || "2000"),
  },

  // Logging
  logLevel: process.env.LOG_LEVEL || "info",
};
