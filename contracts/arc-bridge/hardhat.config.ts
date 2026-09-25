import { HardhatUserConfig } from "hardhat/config";
import "@nomicfoundation/hardhat-toolbox";
import * as dotenv from "dotenv";

dotenv.config({ path: "../../.env" });

const PRIVATE_KEY = process.env.ARC_RELAYER_PRIVATE_KEY || "0x" + "0".repeat(64);
const ARC_TESTNET_RPC = process.env.ARC_TESTNET_RPC || "https://rpc.testnet.arc.io";
const ARCSCAN_API_KEY = process.env.ARCSCAN_API_KEY || "";

const config: HardhatUserConfig = {
  solidity: {
    version: "0.8.20",
    settings: {
      optimizer: { enabled: true, runs: 200 },
    },
  },
  networks: {
    "arc-testnet": {
      url: ARC_TESTNET_RPC,
      accounts: [PRIVATE_KEY],
      chainId: 5042002,
    },
  },
  etherscan: {
    apiKey: ARCSCAN_API_KEY,
  },
};

export default config;
