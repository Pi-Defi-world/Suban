import { ethers, Contract, JsonRpcProvider, EventLog } from "ethers";
import { EventEmitter } from "events";
import { config } from "../config";
import { logger } from "../logger";

const ARC_BRIDGE_ABI = [
  "event PusdBurned(string indexed destChain, address indexed sender, uint256 amount, uint256 nonce)",
  "event PusdMinted(string indexed sourceChain, address indexed recipient, uint256 amount, uint256 nonce, bytes32 sourceTxHash)",
];

export interface ArcBurnEvent {
  type: "burn";
  sourceChain: "arc";
  destinationChain: string;
  sender: string;
  amount: string;
  nonce: number;
  txHash: string;
  blockNumber: number;
  timestamp: number;
}

export class ArcWatcher extends EventEmitter {
  private provider: JsonRpcProvider;
  private bridgeContract: Contract;
  private lastBlock: number = 0;
  private running: boolean = false;
  private pollTimer: NodeJS.Timeout | null = null;

  constructor(bridgeAddress: string) {
    super();
    this.provider = new JsonRpcProvider(
      config.arc.rpcUrl,
      config.arc.chainId
    );
    this.bridgeContract = new Contract(
      bridgeAddress,
      ARC_BRIDGE_ABI,
      this.provider
    );
  }

  async start(fromBlock?: number): Promise<void> {
    if (this.running) return;
    this.running = true;

    // Get current block if not specified
    if (!fromBlock) {
      this.lastBlock = await this.provider.getBlockNumber();
      logger.info(`Arc watcher starting from block ${this.lastBlock}`);
    } else {
      this.lastBlock = fromBlock;
    }

    this.poll();
  }

  stop(): void {
    this.running = false;
    if (this.pollTimer) {
      clearTimeout(this.pollTimer);
      this.pollTimer = null;
    }
  }

  private async poll(): Promise<void> {
    if (!this.running) return;

    try {
      await this.checkForEvents();
    } catch (error) {
      logger.error("Arc watcher error:", error);
    }

    this.pollTimer = setTimeout(
      () => this.poll(),
      config.polling.arc
    );
  }

  private async checkForEvents(): Promise<void> {
    const currentBlock = await this.provider.getBlockNumber();

    if (currentBlock <= this.lastBlock) {
      return;
    }

    logger.debug(
      `Checking Arc blocks ${this.lastBlock + 1} to ${currentBlock}`
    );

    // Query PusdBurned events
    const events = await this.bridgeContract.queryFilter(
      "PusdBurned",
      this.lastBlock + 1,
      currentBlock
    );

    for (const event of events) {
      const burnEvent = this.parseBurnEvent(event as EventLog);
      if (burnEvent) {
        logger.info(
          `Detected burn: ${burnEvent.amount} PUSD from ${burnEvent.sender} (nonce: ${burnEvent.nonce})`
        );
        this.emit("burn", burnEvent);
      }
    }

    this.lastBlock = currentBlock;
  }

  private parseBurnEvent(event: EventLog): ArcBurnEvent | null {
    try {
      const { args, transactionHash, blockNumber } = event;
      if (!args) return null;

      return {
        type: "burn",
        sourceChain: "arc",
        destinationChain: args.destChain,
        sender: args.sender,
        amount: args.amount.toString(),
        nonce: parseInt(args.nonce.toString()),
        txHash: transactionHash,
        blockNumber: blockNumber,
        timestamp: Math.floor(Date.now() / 1000),
      };
    } catch (error) {
      logger.error("Failed to parse Arc burn event:", error);
      return null;
    }
  }
}
