import * as SorobanRpc from "@stellar/stellar-sdk/rpc";
import * as StellarSdk from "@stellar/stellar-sdk";
import { EventEmitter } from "events";
import { config } from "../config";
import { logger } from "../logger";

export interface StellarBurnEvent {
  type: "burn";
  sourceChain: "stellar";
  destinationChain: string;
  sender: string;
  amount: string;
  nonce: number;
  txHash: string;
  ledger: number;
  timestamp: number;
}

export class StellarWatcher extends EventEmitter {
  private server: SorobanRpc.Server;
  private contractId: string;
  private lastLedger: number = 0;
  private running: boolean = false;
  private pollTimer: NodeJS.Timeout | null = null;

  constructor(contractId: string) {
    super();
    this.server = new SorobanRpc.Server(config.stellar.rpcUrl, {
      allowHttp: true,
    });
    this.contractId = contractId;
  }

  async start(fromLedger?: number): Promise<void> {
    if (this.running) return;
    this.running = true;

    // Get current ledger if not specified
    if (!fromLedger) {
      const latestLedger = await this.getLatestLedger();
      this.lastLedger = latestLedger;
      logger.info(`Stellar watcher starting from ledger ${this.lastLedger}`);
    } else {
      this.lastLedger = fromLedger;
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
      logger.error("Stellar watcher error:", error);
    }

    this.pollTimer = setTimeout(
      () => this.poll(),
      config.polling.stellar
    );
  }

  private async checkForEvents(): Promise<void> {
    const currentLedger = await this.getLatestLedger();

    if (currentLedger <= this.lastLedger) {
      return;
    }

    logger.debug(
      `Checking Stellar ledgers ${this.lastLedger + 1} to ${currentLedger}`
    );

    // Get events from contract
    const events = await this.server.getEvents({
      startLedger: this.lastLedger + 1,
      endLedger: currentLedger,
      filters: [
        {
          type: "contract",
          contractIds: [this.contractId],
        },
      ],
    });

    for (const event of events.events) {
      const burnEvent = this.parseBurnEvent(event);
      if (burnEvent) {
        logger.info(
          `Detected burn: ${burnEvent.amount} PUSD from ${burnEvent.sender} (nonce: ${burnEvent.nonce})`
        );
        this.emit("burn", burnEvent);
      }
    }

    this.lastLedger = currentLedger;
  }

  private parseBurnEvent(event: any): StellarBurnEvent | null {
    try {
      // Decode event topics and data
      const topics = event.topic;
      const data = event.value;

      // Check if this is a burn event (topic[0] = "burn")
      if (!topics || topics.length < 2) return null;

      const eventType = StellarSdk.scValToNative(topics[0]);
      if (eventType !== "burn") return null;

      // Parse event data
      const destination = StellarSdk.scValToNative(topics[1]);
      const sender = StellarSdk.scValToNative(data.sender);
      const amount = StellarSdk.scValToNative(data.amount).toString();
      const nonce = parseInt(StellarSdk.scValToNative(data.nonce).toString());

      return {
        type: "burn",
        sourceChain: "stellar",
        destinationChain: destination,
        sender: sender,
        amount: amount,
        nonce: nonce,
        txHash: event.transactionHash,
        ledger: event.ledger,
        timestamp: Math.floor(Date.now() / 1000),
      };
    } catch (error) {
      logger.error("Failed to parse burn event:", error);
      return null;
    }
  }

  private async getLatestLedger(): Promise<number> {
    const response = await this.server.getLatestLedger();
    return response.sequence;
  }
}
