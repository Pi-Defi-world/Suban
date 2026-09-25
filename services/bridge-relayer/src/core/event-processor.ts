import { EventEmitter } from "events";
import { logger } from "../logger";
import { StellarBurnEvent } from "../chains/stellar-watcher";
import { ArcBurnEvent } from "../chains/arc-watcher";
import { TransactionBuilder } from "./tx-builder";

export interface ProcessedEvent {
  id: string;
  sourceChain: "stellar" | "arc";
  status: "pending" | "processing" | "completed" | "failed";
  createdAt: number;
  completedAt?: number;
  txHash?: string;
  error?: string;
}

export class EventProcessor extends EventEmitter {
  private txBuilder: TransactionBuilder;
  private processedEvents: Map<string, ProcessedEvent> = new Map();
  private processingQueue: Array<StellarBurnEvent | ArcBurnEvent> = [];
  private isProcessing: boolean = false;

  constructor() {
    super();
    this.txBuilder = new TransactionBuilder();
  }

  /**
   * Process a burn event from Stellar
   */
  async processStellarBurn(event: StellarBurnEvent): Promise<void> {
    const eventId = `stellar-${event.txHash}-${event.nonce}`;

    if (this.processedEvents.has(eventId)) {
      logger.debug(`Event ${eventId} already processed, skipping`);
      return;
    }

    logger.info(`Processing Stellar burn event: ${eventId}`);

    const processed: ProcessedEvent = {
      id: eventId,
      sourceChain: "stellar",
      status: "processing",
      createdAt: Date.now(),
    };
    this.processedEvents.set(eventId, processed);

    try {
      // TODO: Get recipient address from event or mapping
      const recipient = event.sender; // placeholder

      const txHash = await this.txBuilder.buildArcMint(event, recipient);

      processed.status = "completed";
      processed.completedAt = Date.now();
      processed.txHash = txHash;

      logger.info(`Stellar burn processed: ${eventId} → ${txHash}`);
      this.emit("completed", processed);
    } catch (error: any) {
      processed.status = "failed";
      processed.completedAt = Date.now();
      processed.error = error.message;

      logger.error(`Stellar burn failed: ${eventId}`, error);
      this.emit("failed", processed);
    }
  }

  /**
   * Process a burn event from Arc
   */
  async processArcBurn(event: ArcBurnEvent): Promise<void> {
    const eventId = `arc-${event.txHash}-${event.nonce}`;

    if (this.processedEvents.has(eventId)) {
      logger.debug(`Event ${eventId} already processed, skipping`);
      return;
    }

    logger.info(`Processing Arc burn event: ${eventId}`);

    const processed: ProcessedEvent = {
      id: eventId,
      sourceChain: "arc",
      status: "processing",
      createdAt: Date.now(),
    };
    this.processedEvents.set(eventId, processed);

    try {
      // TODO: Get recipient address from event or mapping
      const recipient = event.sender; // placeholder

      const txHash = await this.txBuilder.buildStellarMint(event, recipient);

      processed.status = "completed";
      processed.completedAt = Date.now();
      processed.txHash = txHash;

      logger.info(`Arc burn processed: ${eventId} → ${txHash}`);
      this.emit("completed", processed);
    } catch (error: any) {
      processed.status = "failed";
      processed.completedAt = Date.now();
      processed.error = error.message;

      logger.error(`Arc burn failed: ${eventId}`, error);
      this.emit("failed", processed);
    }
  }

  /**
   * Check if an event has already been processed
   */
  isProcessed(eventId: string): boolean {
    return this.processedEvents.has(eventId);
  }

  /**
   * Get processing statistics
   */
  getStats(): {
    total: number;
    completed: number;
    failed: number;
    pending: number;
  } {
    let completed = 0;
    let failed = 0;
    let pending = 0;

    for (const event of this.processedEvents.values()) {
      switch (event.status) {
        case "completed":
          completed++;
          break;
        case "failed":
          failed++;
          break;
        case "processing":
          pending++;
          break;
      }
    }

    return {
      total: this.processedEvents.size,
      completed,
      failed,
      pending,
    };
  }
}
