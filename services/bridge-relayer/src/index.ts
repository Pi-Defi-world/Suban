import { config } from "./config";
import { logger } from "./logger";
import { StellarWatcher } from "./chains/stellar-watcher";
import { ArcWatcher } from "./chains/arc-watcher";
import { EventProcessor } from "./core/event-processor";

class BridgeRelayer {
  private stellarWatcher: StellarWatcher;
  private arcWatcher: ArcWatcher;
  private processor: EventProcessor;
  private running: boolean = false;

  constructor() {
    this.stellarWatcher = new StellarWatcher(config.stellar.bridgeContract);
    this.arcWatcher = new ArcWatcher(config.arc.bridgeContract);
    this.processor = new EventProcessor();

    this.setupEventHandlers();
  }

  private setupEventHandlers(): void {
    // Stellar burn events → mint on Arc
    this.stellarWatcher.on("burn", async (event) => {
      logger.info(
        `Stellar burn detected: ${event.amount} PUSD from ${event.sender}`
      );
      await this.processor.processStellarBurn(event);
    });

    // Arc burn events → mint on Stellar
    this.arcWatcher.on("burn", async (event) => {
      logger.info(
        `Arc burn detected: ${event.amount} PUSD from ${event.sender}`
      );
      await this.processor.processArcBurn(event);
    });

    // Processing events
    this.processor.on("completed", (event) => {
      logger.info(`Bridge transfer completed: ${event.id}`);
    });

    this.processor.on("failed", (event) => {
      logger.error(`Bridge transfer failed: ${event.id} - ${event.error}`);
    });
  }

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;

    logger.info("Starting bridge relayer...");
    logger.info(`Stellar RPC: ${config.stellar.rpcUrl}`);
    logger.info(`Arc RPC: ${config.arc.rpcUrl}`);
    logger.info(`Fee percentage: ${config.fees.percentage}%`);

    // Start watchers
    await this.stellarWatcher.start();
    await this.arcWatcher.start();

    logger.info("Bridge relayer started");

    // Log stats periodically
    setInterval(() => {
      const stats = this.processor.getStats();
      logger.info("Relayer stats:", stats);
    }, 60000); // every minute
  }

  stop(): void {
    this.running = false;
    this.stellarWatcher.stop();
    this.arcWatcher.stop();
    logger.info("Bridge relayer stopped");
  }
}

// Main entry point
async function main() {
  const relayer = new BridgeRelayer();

  // Handle graceful shutdown
  process.on("SIGINT", () => {
    logger.info("SIGINT received, shutting down...");
    relayer.stop();
    process.exit(0);
  });

  process.on("SIGTERM", () => {
    logger.info("SIGTERM received, shutting down...");
    relayer.stop();
    process.exit(0);
  });

  await relayer.start();
}

main().catch((error) => {
  logger.error("Failed to start relayer:", error);
  process.exit(1);
});
