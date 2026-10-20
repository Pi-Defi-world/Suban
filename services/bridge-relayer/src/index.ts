import { config } from "./config";
import { logger } from "./logger";
import { StellarWatcher } from "./chains/stellar-watcher";
import { ArcWatcher } from "./chains/arc-watcher";
import { EventProcessor } from "./core/event-processor";
import {
  eventsSeen,
  lastEventTimestamp,
  relayerRunning,
  startMetricsServer,
  transfers,
  updateProcessorStats,
} from "./metrics";
import http from "http";

class BridgeRelayer {
  private stellarWatcher: StellarWatcher;
  private arcWatcher: ArcWatcher;
  private processor: EventProcessor;
  private running: boolean = false;
  private metricsServer: http.Server | null = null;

  constructor() {
    this.stellarWatcher = new StellarWatcher(config.stellar.bridgeContract);
    this.arcWatcher = new ArcWatcher(config.arc.bridgeContract);
    this.processor = new EventProcessor();

    this.setupEventHandlers();
  }

  private setupEventHandlers(): void {
    // Stellar burn events → mint on Arc
    this.stellarWatcher.on("burn", async (event) => {
      eventsSeen.inc({ chain: "stellar" });
      lastEventTimestamp.set({ chain: "stellar" }, Math.floor(Date.now() / 1000));
      logger.info(
        `Stellar burn detected: ${event.amount} PUSD from ${event.sender}`
      );
      await this.processor.processStellarBurn(event);
    });

    // Arc burn events → mint on Stellar
    this.arcWatcher.on("burn", async (event) => {
      eventsSeen.inc({ chain: "arc" });
      lastEventTimestamp.set({ chain: "arc" }, Math.floor(Date.now() / 1000));
      logger.info(
        `Arc burn detected: ${event.amount} PUSD from ${event.sender}`
      );
      await this.processor.processArcBurn(event);
    });

    // Processing events
    this.processor.on("completed", (event) => {
      transfers.inc({
        direction:
          event.sourceChain === "stellar" ? "stellar->arc" : "arc->stellar",
        status: "completed",
      });
      updateProcessorStats(this.processor.getStats());
      logger.info(`Bridge transfer completed: ${event.id}`);
    });

    this.processor.on("failed", (event) => {
      transfers.inc({
        direction:
          event.sourceChain === "stellar" ? "stellar->arc" : "arc->stellar",
        status: "failed",
      });
      updateProcessorStats(this.processor.getStats());
      logger.error(`Bridge transfer failed: ${event.id} - ${event.error}`);
    });
  }

  async start(): Promise<void> {
    if (this.running) return;
    this.running = true;
    relayerRunning.set(1);
    this.metricsServer = startMetricsServer(config.metricsPort);

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
      updateProcessorStats(stats);
      logger.info("Relayer stats:", stats);
    }, 60000); // every minute
  }

  stop(): void {
    this.running = false;
    relayerRunning.set(0);
    this.stellarWatcher.stop();
    this.arcWatcher.stop();
    if (this.metricsServer) {
      this.metricsServer.close();
      this.metricsServer = null;
    }
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
