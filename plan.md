SUBAN
Master Architecture, Product Strategy & Roadmap
Canonical planning document • 2026–2027
Financial Infrastructure for Pi Network
Revision 3 — Unified edition • October 2026
Document Status
Revision 3 — Unified edition. This edition consolidates three earlier drafts into one canonical document: the original Master Architecture (full strategic and architectural scope), the UPDATED revision of 3 October 2026 (which added the now-public suban-controller repository and a source-backed repository map), and the COMPREHENSIVE merge (detailed controller, oracle-trust, data, security, operations and roadmap sections). Overlapping material has been merged once, in its proper section, and the controller is now documented as a verified component rather than a “to verify” item.
Field
Value
Status
Canonical strategy / architecture draft
Scope
Suban platform, repositories, infrastructure, financial extensions
Planning horizon
October 2026 – December 2027
Existing foundation
RPC, Horizon/API, controller (price aggregation and oracle publication), contract infrastructure, contract finder, escrow, price oracle and related services
Repositories covered
Suban, suban-rpc, Suban-api, suban-controller (all public; see Sections 3 and 6)
Primary principle
Build reusable infrastructure first; launch financial products only where demand is demonstrated
Important distinction
Aquarius/Blend are candidate product hypotheses, not validated strategic commitments


Source note: Current repository facts in this document are based on the public repositories reviewed on October 3, 2026, including the now-public suban-controller repository. Where a repository or implementation detail could not be verified publicly, it is explicitly marked as a proposal or “to verify.”

Table of Contents
1. Executive Summary
2. What Suban Is
3. Current State: What Exists Today
4. Strategic Thesis
5. Product Architecture
6. Repository & Ownership Map (incl. suban-controller)
7. Core Platform Modules
8. Financial Infrastructure Modules
9. Proposed DeFi Products
10. API & Module Boundaries
11. Data Architecture
12. Security Architecture
13. Reliability & Operations
14. Revenue Model
15. Product Validation Framework
16. 2026–2027 Roadmap
17. Milestones & Exit Criteria
18. Dependencies
19. Risks
20. Non-Goals
21. Definition of Done
22. Immediate Next Actions
Appendix A — Current Repository Evidence
Appendix B — Terminology
Appendix C — Strategic One-Liner
Appendix D — Canonical Architecture Flow

1. Executive Summary
Suban should be developed as a layered blockchain and financial infrastructure platform for Pi Network, not as a collection of unrelated DeFi applications. It should evolve from a collection of Pi blockchain services into a coherent infrastructure platform that developers and financial applications can build on. The existing foundation already includes a Pi JSON-RPC server, a Horizon API wrapper, a price-oracle/data controller, smart-contract infrastructure, SDK/services, escrow functionality, and a price oracle. The strategy is not to discard those systems or immediately turn Suban into a single DeFi application.
The strategic objective is to add layers around the existing core: indexed blockchain data, financial market data, risk services, execution services, capital-management primitives, and eventually shared liquidation/settlement infrastructure.
The platform should create two complementary businesses: (1) infrastructure services consumed by external developers and protocols, and (2) financial products operated by the Suban team when market evidence supports them.
The newly public suban-controller repository materially changes the architecture map. It is not merely a future controller concept: it is an implemented TypeScript service that aggregates Pi price feeds from four external sources, performs staleness, outlier and circuit-breaker handling, exposes on-chain analytics through local Horizon, proxies Horizon traffic, and can periodically push aggregated prices into an on-chain oracle contract. This makes the controller the clearest current implementation of Suban’s oracle/data-control plane.
The strategic direction therefore becomes: stabilize the existing infrastructure, turn controller data into a dependable data/financial platform, add execution capabilities only when there is real liquidity to route, and deploy capital-bearing products only after measurable demand and security validation.
The central rule: infrastructure may be built ahead of demand where it is broadly reusable; capital-intensive DeFi products must be validated before significant deployment.
2. What Suban Is
Working definition: Suban is blockchain and financial infrastructure for Pi Network: a platform that provides reliable access to Pi network state, smart contracts, indexed data, financial data, risk intelligence, transaction execution capabilities, and reusable financial primitives.
Canonical positioning: “Suban is the infrastructure layer that lets Pi applications read, understand, execute and eventually manage financial activity without rebuilding the underlying infrastructure themselves.”
Public shorthand: “Suban — Financial Infrastructure for Pi Network.”
Pi Applications
      │
      ▼
┌─────────────────────────────────────────────┐
│                   SUBAN                     │
│                                             │
│  Core → Data → Financial → Execution        │
│                  → Capital → Protection     │
└─────────────────────────────────────────────┘
      │
      ▼
Pi Network
Suban is therefore not defined by one protocol. RPC is a foundation. Horizon/API is a foundation. Escrow is a product/primitives layer. The future value comes from composing these capabilities into a platform — the reusable infrastructure underneath many Pi applications.
3. Current State: What Exists Today
The current repositories establish a real base that should be preserved. All four repositories are public and were reviewed on October 3, 2026.
Repository
Verified current role
Strategic role
Status
Suban
Main Suban platform: Rust workspace with contracts, crates, docs, services, SDK, and frontend/escrow-viewer components
Core platform, contracts, SDK, shared services and supporting product infrastructure
Public; existing
suban-rpc
Pi JSON-RPC server; network state, transactions, status/history, preflight simulation, Soroban interactions; monitoring and Pi DApp assets
Blockchain connectivity and execution foundation
Public; existing
Suban-api
Horizon API wrapper with caching, rate limiting, enhanced endpoints and raw Horizon proxy
Application data access layer
Public; existing
suban-controller
TypeScript data-oracle/controller: multi-source price aggregation, chain analytics, Horizon proxy and optional on-chain oracle push
Oracle / data-control plane
Public; newly verified


The Suban repository publicly contains contracts, crates, docs, SDK, services and an escrow viewer. The RPC repository describes itself as part of the Suban Data Platform and exposes JSON-RPC plus an internal admin API. The API repository provides summary, accounts, transactions, ledgers, assets and raw Horizon proxying. The controller repository is described in detail in Section 6.1.
Repository URLs:
https://github.com/Pi-Defi-world/Suban
https://github.com/Pi-Defi-world/suban-rpc
https://github.com/Pi-Defi-world/Suban-api
https://github.com/Pi-Defi-world/suban-controller
Existing price oracle: Suban already has a price oracle. The roadmap therefore treats “price oracle” as an existing capability and proposes extending the surrounding financial intelligence, not rebuilding the oracle from zero. The suban-controller repository is the clearest current implementation of that oracle’s data pipeline (see Sections 6.1 and 7.5).
4. Strategic Thesis
Pi does not need Suban to guess which DeFi application will win. Pi applications will require infrastructure regardless of whether the eventual winners are lending, liquidity management, payments, marketplaces, treasury systems, or other financial applications.
The strategic thesis is therefore:
Build infrastructure that many applications need; then use that infrastructure to launch only the financial products that demonstrate demand.
Developer need
      ↓
Blockchain access
      ↓
Indexed state
      ↓
Financial state
      ↓
Price + risk intelligence
      ↓
Simulation + execution
      ↓
Capital management
      ↓
Applications
This approach also directly addresses a known failure mode: building technically interesting products that do not acquire durable users.
5. Product Architecture
Suban should be organized into six logical layers.
Layer
Name
Primary responsibility
L0
Suban Core
RPC, Horizon/API, contracts, SDK, escrow, node connectivity
L1
Suban Data
Indexing, normalized blockchain data, events, historical data, webhooks
L2
Suban Financial
Existing price oracle + market data + TWAP + liquidity/volatility/risk calculations
L3
Suban Execution
Quotes, route discovery, simulation, transaction construction and execution services
L4
Suban Capital
Liquidity vaults, strategy vaults, capital management and eventually lending
L5
Suban Protection
Liquidation infrastructure, monitoring, settlement/financial state reconciliation


These are product boundaries, not necessarily separate repositories. Implementation should follow repository ownership and operational needs.
5.1 Current Implementation by Layer
The table below maps each layer to what exists today and to its strategic role.
Layer
Current implementation
Strategic role
L0 — Core Access
RPC, Horizon/API, contract infrastructure and existing services
Reliable base layer for Pi application developers
L1 — Data
Controller chain analytics, Horizon access; future indexing/events/webhooks
Normalized blockchain data without every app running its own ingestion stack
L2 — Financial Data
Existing multi-source Pi price aggregation and on-chain oracle push
Price, market, liquidity, volatility and risk intelligence
L3 — Execution
RPC transaction submission/simulation; future quote/route services
Best execution and transaction construction across real liquidity sources
L4 — Capital
Future vault/strategy/lending modules
Managed capital only where demand and economics are demonstrated
L5 — Protection / Settlement
Future liquidation, monitoring and settlement services
Shared safety and financial-state coordination across protocols


6. Repository & Ownership Map
Component
Current repository
Future responsibility
Core contracts
Suban
Smart contracts, shared primitives and protocol-specific contracts
SDK
Suban
Typed client access to Suban services/contracts
Escrow
Suban
Escrow contracts, SDK integration and escrow viewer
RPC
suban-rpc
Pi JSON-RPC, transaction submission, status, simulation and contract interaction
Horizon/API
Suban-api
REST/data access, caching, rate limiting, Horizon compatibility
Controller
suban-controller
Market-data aggregation, source health, controller analytics, Horizon proxy and oracle-publication orchestration (verified — see 6.1)
Financial indexing
Suban-api or dedicated service after scale review
Normalized market/financial data
Oracle
Existing Suban oracle + suban-controller + future financial services
Price feeds first; broader derived metrics later
Execution
New service/module under Suban architecture
Quote, route, simulation, transaction construction
Capital
New contracts/services
Vaults and capital strategies only after validation


Repository consolidation rule: do not split every new feature into a new repository. Create a new repository only when deployment lifecycle, security boundary, language/runtime, or operational ownership genuinely requires it.
6.1 suban-controller — Detailed Verified Architecture
The newly public suban-controller repository is now a first-class part of the canonical Suban architecture. It should not be documented as a hypothetical controller or merely as a future oracle component. The implementation is an off-chain TypeScript/Node.js control and data service that connects external market-price sources, local Pi/Horizon data, aggregation/risk logic and optional on-chain oracle publication.
This distinction matters architecturally: suban-controller is not the replacement for suban-rpc or Suban-api. It sits above the chain-access layer and consumes infrastructure from it while adding market-data aggregation, chain analytics and oracle-publication orchestration.
6.1.1 Controller Responsibilities
Market-data ingestion from multiple external price sources (MEXC, CoinGecko, OKX and Bitget).
Price normalization and aggregation.
Source freshness/staleness validation.
Median-based outlier detection and filtering.
Weighted aggregation of valid observations.
Confidence scoring based on source coverage and price consistency.
Price-deviation monitoring and circuit-breaker behavior, including last-committed-price fallback.
Current source-health reporting.
Local-Horizon-backed chain statistics and recent chain-data access (latest ledgers, account information/top accounts, recent transactions).
Horizon request proxying through a transparent /horizon/* proxy to the local Horizon service.
Optional periodic publication of an aggregated price/confidence value to an on-chain oracle contract using the Soroban CLI.
Operational endpoints for health, source status, circuit-breaker state, deviation alerts and oracle-push statistics.
Caching for price and chain data, with graceful shutdown and error logging.
6.1.2 Runtime / Technology Boundary
The public repository identifies itself as “Suban Controller” and describes its purpose as a data oracle for Pi Network; it is part of the Suban Data Platform. The package identity is @suban/controller (version 1.0.0 in package.json). Its implementation uses an HTTP server stack with Express, Axios for external/service requests, Node-cache for short-lived caching, Winston for logging and Jest for testing. The service therefore belongs to the application/data-control plane rather than the Rust smart-contract plane or the Go/Rust RPC execution plane.
6.1.3 Controller Data Flow
External price sources
  → source adapters
  → PriceAggregator
  → freshness validation
  → outlier filtering
  → weighted aggregation
  → confidence calculation
  → cache
  → API consumers and/or OraclePushService
  → on-chain oracle
Separately: local Horizon/Pi infrastructure → chain-data service → normalized chain statistics/ledger/account/transaction responses → controller API and future Suban financial-data pipelines.
6.1.4 Price Aggregation Model
The current controller implementation documents four market sources: MEXC, CoinGecko, OKX and Bitget. The documented source weights are MEXC 3.0x, CoinGecko 1.5x, OKX 2.0x and Bitget 2.0x. These values are implementation configuration, not a universal financial truth; they should therefore be versioned, monitored and reviewed as source quality changes.
A source observation is rejected when it is stale according to the configured freshness policy.
The aggregator requires a minimum number of usable sources before producing a trusted aggregate.
Median-based filtering is used to identify observations that deviate materially from the source cluster.
The remaining observations are combined using configured weights.
A confidence value is calculated from source availability and price consistency.
The last committed value and deviation controls provide a safety boundary when current observations become abnormal: the circuit breaker engages when the aggregated price deviates beyond the configured threshold from the last committed price.
We still have a problem with the sources of price because it is meant to be generated by multiple sources, it has to be for usdc, usd,ousd and usdt 
6.1.5 Controller API Surface
Endpoint
Purpose
Classification
GET /api/v1/price
Current aggregated Pi price
Public data
GET /api/v1/sources
Source status and source-level information
Operational / data
GET /api/v1/health
Controller, source and circuit-breaker health
Operational
POST /api/v1/price/commit
Controlled commit of the last-known price
Privileged / admin-sensitive
GET /api/v1/circuit-breaker
Current circuit-breaker state
Operational
POST /api/v1/circuit-breaker/reset
Reset the circuit breaker
Privileged / admin-sensitive
GET /api/v1/deviation-alerts
Recent price-deviation alerts
Operational / risk
GET /api/v1/oracle/stats
On-chain oracle push/publication statistics
Operational
GET /api/v1/chain/stats
Network statistics from local Horizon
Data
GET /api/v1/chain/ledgers
Latest / recent ledgers
Data
GET /api/v1/chain/accounts/:id
Account information
Data
GET /api/v1/chain/accounts
Account listing / top accounts
Data
GET /api/v1/chain/transactions
Recent transactions
Data
ALL /horizon/*
Proxy to local Horizon
Infrastructure


6.1.6 Public vs Privileged Controller Surface
The controller has two fundamentally different classes of operations. Read-only data operations can be made public behind normal rate limits and observability. Control operations such as price commit, circuit-breaker reset and oracle publication must be treated as privileged operations. They should never be exposed as unrestricted public endpoints.
The controller currently contains administrative endpoints and an oracle-push path. Production deployment should separate public read access from privileged control paths through network policy, authentication/authorization, secret isolation and ideally separate processes or services.
Production boundary: public API gateway → read-only controller routes. Separately: private control network → authenticated controller control routes → signing/oracle publisher. This separation should be reflected in deployment topology, secrets, authorization and monitoring.
6.1.7 OraclePushService
The controller contains an explicit oracle-push service that can take the aggregated price and confidence information and publish it to an on-chain oracle contract through Soroban tooling (Soroban CLI). The implementation scales the price for on-chain representation, publishes a confidence value and records push statistics. This means the controller is not simply an API that reports an off-chain price; it can participate in the lifecycle of the existing Suban oracle.
That capability creates a critical trust boundary. A compromised controller or publisher identity could potentially publish an incorrect value. The production design must therefore isolate signing credentials, restrict publisher authority, monitor publication results, enforce sanity checks before publication and provide a controlled recovery path. The on-chain oracle contract interface and publisher authority used by OraclePushService must be verified before the path is treated as production-grade.
6.1.8 Chain-Data Responsibilities
The controller exposes chain-data helpers for network statistics, recent ledgers, account information, account listings and recent transactions using the local Horizon service. These functions are useful for operational dashboards and early analytics, but they should not become a second competing general-purpose Horizon API.
Recommended boundary: Suban-api remains the canonical developer-facing Horizon/API wrapper; suban-controller consumes or proxies chain information where required for controller logic and operational surfaces. Shared data models should be normalized to avoid incompatible representations of the same Pi objects.
6.1.9 Controller Failure Modes
One external price source unavailable.
Multiple price sources unavailable.
Source returns stale data.
Source returns an extreme outlier.
Source disagreement produces low confidence.
Aggregated price deviates beyond configured circuit-breaker threshold.
Local Horizon unavailable or delayed.
On-chain oracle transaction fails.
Oracle publisher credentials unavailable/expired.
Controller process crashes or restarts.
External API rate limits or changes response format.
6.1.10 Controller Observability
Per-source request success/failure rate.
Per-source response latency.
Source freshness age.
Source-to-median deviation.
Aggregate confidence.
Number of usable sources.
Circuit-breaker activations.
Oracle publication attempts/successes/failures.
Last successful on-chain publication.
Controller uptime and restart count.
Horizon dependency health.
6.1.11 Controller Evolution
The controller should evolve into Suban’s Financial Data Control Plane rather than becoming a monolithic application. Market-source adapters, chain-data adapters, aggregation logic, risk calculations and oracle publication should have explicit interfaces so each can be tested and replaced independently.
Long-term target: external market data + Pi-native on-chain observations → validated financial dataset → oracle/risk services → API/SDK → consuming applications and contracts.
6.2 Repository Boundary Matrix
Suban core owns reusable contracts, Rust crates, SDK/services and product primitives. suban-rpc owns JSON-RPC access and execution/simulation. Suban-api owns the developer-facing Horizon wrapper and enhanced API surface. suban-controller owns external market-data aggregation, controller analytics and oracle publication orchestration. Future financial-data/risk/execution services should consume these boundaries rather than duplicate them.
Component
Owns
Should not own
suban-controller
Market-source aggregation, source health, chain-data read aggregation, Horizon proxy, oracle publication orchestration
Core RPC transaction gateway, broad API product logic, capital strategy logic
Suban-api
Developer-facing HTTP/Horizon wrapper, caching, rate limiting, enhanced API endpoints
Oracle signing/publishing authority
suban-rpc
JSON-RPC execution/state/simulation interface
External market price methodology
Suban core
Shared Rust/contracts/services and platform-level primitives
Duplicating the controller’s market-source adapters unnecessarily


This boundary is a target architecture: exact module ownership should be finalized from source-level dependency mapping before large refactors.
7. Core Platform Modules
7.1 RPC
Purpose: reliable programmatic access to Pi network state and transaction execution. suban-rpc is the execution gateway.
Network-state queries
Transaction submission
Transaction status/history
Preflight/simulation
Smart-contract interaction (Soroban)
Operational/admin metrics kept separate from public RPC
The current RPC already documents these responsibilities.
7.2 Horizon/API
Purpose: developer-friendly access to indexed/network data. Suban-api wraps Horizon with caching, rate limiting and enhanced endpoints, providing a developer-facing HTTP surface (while the controller can also proxy the project’s local Horizon).
Accounts
Transactions
Ledgers
Assets
Network summary
Raw Horizon proxy
Caching and rate limiting
The next step is to expand this from a Horizon convenience layer into a normalized Suban data platform.
7.3 Contract Discovery
Purpose: make deployed contracts discoverable and usable by developers.
Contract metadata
Network/environment
Contract address
ABI/interface metadata where available
Known versions/deployments
Verification metadata
Contract discovery should become a first-class developer service rather than a utility hidden inside individual applications.
7.4 Escrow
Purpose: reusable conditional/time-based transaction primitives and user-facing escrow tooling.
Contract-level escrow
State inspection
Integration with Suban data services
Future webhook/event notifications
7.5 Existing Price Oracle
Purpose: provide price information to applications that require valuation.
This component already exists and should remain a foundational primitive. Future work should improve source quality, update policy, historical availability, confidence metadata and integration with risk calculations rather than duplicating the same oracle. The roadmap must therefore refer to future work as Oracle Extensions / Financial Oracle Layer, not “build an oracle from scratch.”
Current flow:
External price sources
  → PriceAggregator
  → staleness / outlier checks
  → weighted aggregation
  → confidence score
  → cache
  → optional on-chain push
  → consuming contracts / apps
Currently implemented protections:
Source freshness checks
Minimum-source requirement
Median-based outlier filtering
Weighted averaging
Confidence scoring
Deviation alerts
Circuit breaker
Last committed price fallback
Source health reporting
Planned Oracle Extensions are described in Section 8.2.
7.6 Controller (Data-Control Plane)
The controller should be treated as the data-control plane: external market inputs + local chain observations + aggregation/risk logic + optional on-chain oracle publication. Its verified architecture is documented in Section 6.1.
7.7 Contracts / Core Platform
The Suban repository remains the place to map contract and shared Rust components. Individual contract responsibilities should be kept explicit and should not be conflated with the controller’s off-chain responsibilities.
8. Financial Infrastructure Modules
Financial infrastructure should extend the already-working oracle rather than duplicate it.
8.1 Suban Financial Data
Adds financial meaning to the existing blockchain data layer.
Asset prices
Historical prices
Pool discovery
Pool liquidity
Trading volume
Trades
Liquidity-provider positions
Financial events
Portfolio/position views
This is not yet a claim that all of these data sources exist on Pi today. They are target capabilities whose implementation depends on available on-chain sources and ecosystem activity.
8.2 Suban Oracle Extension
The existing oracle remains the price source. Extend around it with:
TWAP calculations where sufficient data exists and source quality and liquidity data justify it
Historical price series
Source freshness, provenance and timestamps
Confidence/quality metadata exposed consistently
Liquidity-aware pricing
Outlier detection and manipulation monitoring
Circuit breakers, with governance and recovery procedures
Versioned oracle methodology
The output should communicate not only a price but the quality and freshness of that price.
8.3 Suban Risk Engine
Purpose: transform price + liquidity + blockchain state into risk information.
Collateral valuation
Health-factor calculations
Liquidation-price calculations
Liquidity risk
Volatility
Concentration
Pool health
Asset-risk metadata
Risk is a derived service. It should consume the existing price oracle rather than replace it.
8.4 Suban Execution
Purpose: make financial transaction execution easier and safer.
Quote
Route discovery
Price-impact calculation
Simulation
Transaction construction
Execution
A router/aggregator should only be promoted to a major product once Pi has multiple meaningful liquidity sources. Before that, the simulation/quote infrastructure can still be useful.
8.5 Controller-to-Financial-Layer Integration
The controller is the first concrete implementation of the financial-data control plane. The future financial layer should consume its validated price observations rather than create a parallel price methodology.
Recommended pipeline: suban-controller → normalized financial-data contract → historical storage/index → Suban Financial API → Risk Engine / Router / Vaults / Lending / Liquidation services.
The controller owns source collection and source-quality logic.
The financial-data layer owns persistence, historical querying and standardized schemas.
The risk layer owns derived financial-risk calculations.
The execution layer owns quote, simulation, routing and transaction construction.
Contracts own deterministic on-chain state and enforce on-chain rules.
8.6 Financial Data Quality Model
Every published financial datum should carry enough metadata for a downstream consumer to decide whether it is safe to use.
value
asset/pair identifier
source or source-set
observation timestamp
aggregation timestamp
freshness age
confidence/quality score
methodology/version
circuit-breaker state where relevant
data provenance
network/environment
8.7 Oracle Trust Model
Suban should treat oracle data as a trust pipeline rather than a single number. A consuming protocol should be able to distinguish a fresh, high-confidence observation from a stale or degraded observation.
Recommended oracle states: HEALTHY → DEGRADED → CIRCUIT-BREAKER → RECOVERY. State transitions should be observable and documented. Contracts that depend on the oracle should define what they do under each state rather than assuming an oracle value is always valid.
8.8 Risk Engine Inputs
Validated price and confidence.
Historical volatility.
Liquidity depth.
Trading volume.
Concentration.
Collateral/liability state.
Asset-specific risk parameters.
Protocol-specific thresholds.
The risk engine should never silently manufacture missing data. Missing or low-quality inputs should produce an explicit degraded result or block a risk-sensitive operation.
8.9 Capability Build Conditions
Capability
Purpose
Build condition
Oracle Extensions
Historical data, freshness/provenance, confidence, TWAP where justified
After data quality is measured
Liquidity Analytics
TVL/liquidity, volume, price impact, concentration and volatility
When meaningful pools exist
Risk Engine
Collateral/price/liquidity risk calculations
When a real protocol needs them
Market Data API
Unified financial data for Pi applications
Once indexed market data is stable
Peg/Reserve Monitoring
Monitor stable-asset price and related market conditions
Only for assets with observable, supported data


9. Proposed DeFi Products
These are candidates, not commitments.
9.1 Automated Liquidity Vault
A managed strategy that allocates user-provided liquidity according to measurable rules.
User capital
     ↓
Liquidity Vault
     ↓
Strategy engine
   ├─ volume
   ├─ fees
   ├─ liquidity
   ├─ volatility
   └─ price impact
     ↓
Liquidity allocation
     ↓
Trading fees / strategy returns
The product must not advertise yield that is primarily created by unsustainable token emissions. Every strategy must identify its source of return.
9.2 Liquidity Optimization
This can initially be a service rather than a public vault. Suban analyzes pools and recommends or automatically executes allocation changes under explicit strategy rules. It starts as analytics/strategy infrastructure; a public capital vault should come only after pool activity, LP demand and fee economics are measurable.
It becomes valuable when there is enough liquidity fragmentation or enough pool activity to make optimization economically meaningful.
9.3 Suban Liquidation Network
A shared liquidation service for lending protocols.
Lending protocol
      ↓
Suban Risk
      ↓
unsafe position
      ↓
Liquidation opportunity
      ↓
Liquidator
      ↓
Suban Execution
Position monitoring
Health-factor calculation
Liquidation event detection
Liquidation opportunity distribution
Transaction construction/simulation
Execution infrastructure
This should be built after lending demand exists. It becomes valuable when multiple lending markets need monitoring/execution. It is an infrastructure opportunity, not a reason to manufacture a lending market.
9.4 Pi DeFi Router / Aggregator
A best-execution layer across meaningful Pi liquidity venues.
Discover routes
Compare output
Calculate price impact
Estimate fees
Simulate
Construct execution transaction
Do not build this as another DEX. It should route across existing liquidity, and only after multiple meaningful liquidity sources exist.
9.5 Suban Settlement
Previously called the “DeFi clearing layer.” This name is preferable because the goal is to normalize financial state across applications — a future normalization/reconciliation layer for deposits, withdrawals, positions, fees, protocol events and settlement status.
Deposits
Withdrawals
Positions
Fees
Contract events
Settlement status
Cross-application financial state
This becomes more valuable as multiple financial applications use Suban.
9.6 Capital Vaults
A generic strategy framework that can later host liquidity, lending or other strategies.
Capital
   ↓
Vault
   ├─ Liquidity strategy
   ├─ Lending strategy
   └─ Future validated strategies
The framework should be modular so an unsuccessful strategy can be retired without killing the entire capital platform.
9.7 Lending
Lending is a candidate application, not a prerequisite for Suban.
Requires reliable price infrastructure
Requires collateral with meaningful liquidity
Requires borrow demand
Requires stable/borrowable asset liquidity
Requires liquidation liquidity
Requires audits and strong risk controls (conservative risk parameters)
Aquarius and Blend should therefore be recorded as design references/candidate implementations, not as the official Suban roadmap. They should not be treated as validated simply because they are attractive designs.
10. API & Module Boundaries
Recommended public service boundary:
Service
Example capability
Consumer
RPC
JSON-RPC / network / transaction / simulation
Wallets, DApps, SDKs
Data API
accounts, ledgers, transactions, assets, events
Apps, explorers, analytics
Contract API
contract discovery/metadata
Developers
Financial API
prices, pools, volume, liquidity
DeFi apps
Oracle
current price / TWAP / source quality
Contracts, risk systems
Risk API
collateral value, health, liquidation price
Lending/vaults
Execution API
quote, route, simulate, build
DEXs, wallets, apps
Capital API
vaults, positions, strategies
Users/protocols
Protection API
monitoring, liquidation events, alerts
Protocols/liquidators


Public APIs should be versioned. Internal control-plane APIs should never be exposed accidentally as public developer APIs.
10.1 Canonical Service Responsibilities
Suban should maintain a clean distinction between data access, data interpretation, execution and privileged control.
suban-rpc: network/RPC execution and supported chain-state operations.
Suban-api: developer-facing HTTP/Horizon wrapper, caching, rate limiting and normalized convenience endpoints.
suban-controller: external market-source aggregation, source health, controller analytics, Horizon-dependent controller data and oracle publication orchestration.
Suban core: contracts, shared Rust crates, SDK, services and platform-level primitives.
Financial Data: persistent/indexed financial datasets and derived market views.
Risk: financial-risk calculations and protocol-specific risk interfaces.
Execution: quotes, route selection, simulation and transaction construction.
Capital: vault and strategy contracts/services.
Protection: liquidation monitoring/execution and settlement/reconciliation.
A common API contract should also be defined between suban-controller, Suban-api and suban-rpc to avoid overlapping or ambiguous data surfaces.
10.2 API Versioning
Public APIs should use explicit version namespaces such as /api/v1. Breaking changes require a new version. Internal control-plane APIs can evolve independently but must be authenticated and documented.
10.3 Authentication and Rate Limits
Public read services should support anonymous or API-key access according to the product tier. Premium/high-volume access should use API keys, quotas and usage accounting. Privileged control endpoints require stronger authentication and must not share the same exposure model as public reads.
10.4 Data Contracts
Suban should define canonical schemas for Asset, PriceObservation, AggregatedPrice, Confidence, Ledger, Transaction, Account, Pool, Market, Position, RiskAssessment, Quote and ExecutionRequest. These schemas should be versioned independently from implementation languages.
11. Data Architecture
Pi Node / Horizon / Contracts      (+ external market sources)
            │
            ▼
      Ingestion layer
            │
            ├── raw blockchain state
            ├── normalized events
            └── contract state
            │
            ▼
      Suban data store
            │
   ┌────────┼───────────────┐
   ▼        ▼               ▼
 Data    Financial      Historical
 API       data         analytics
            │
            ▼
      Oracle / Risk
            │
            ▼
        Execution
            │
            ▼
         Capital
Target data pipeline: Pi nodes/Horizon + external market sources → ingestion → normalization → validation → indexed storage/cache → public API → financial/risk services → execution/contract consumers.
The architecture should distinguish raw source data from derived financial data. Every derived value should retain provenance, timestamp/freshness, source and calculation version where practical.
11.1 Source vs Derived Data
Suban must distinguish raw observations from derived values. Raw chain events and external price observations should be retained or reproducible where practical. Derived values such as confidence, TWAP, volatility and risk scores must identify the calculation methodology and input window.
11.2 Contract-State Retrieval Constraint
The current build has a known limitation around getLedgerEntries: the tested implementation rejects the key formats currently being supplied, including plain account:. Until this is resolved with a tested compatible encoding/path, contract storage reads must not be advertised as a reliable general-purpose capability, and getLedgerEntries should be kept as a documented compatibility limitation until a tested build supports the required contract-state key format.
The architectural response should be an explicit compatibility layer and, where necessary, dedicated event/contract-state indexing. The architecture should prefer supported Horizon/RPC/indexing paths. The team should not make every downstream application depend on a fragile RPC method whose behavior has not been verified on the deployed Pi build.
11.3 Indexing Strategy
Capture supported ledger/transaction/operation/event data.
Normalize records into stable internal schemas.
Maintain block/ledger checkpoints.
Make ingestion idempotent.
Support replay/re-index from a known checkpoint.
Track source lag and indexing completeness.
Separate raw ingestion tables from derived financial tables.
Expose data freshness to API consumers.
11.4 Webhooks
Webhooks should eventually allow applications to subscribe to events such as transaction confirmation, escrow state changes, contract events, oracle state changes, liquidation opportunities and other validated Suban events. Delivery must be idempotent and retryable.
11.5 Data-Platform Priorities
Event indexing
Normalized account/transaction/operation data
Contract metadata/discovery
Historical queries
Webhooks
Usage metrics
API keys and rate tiers
Reconciliation and data-quality checks
12. Security Architecture
Financial infrastructure has a different security standard from ordinary application APIs.
Area
Requirement
RPC
Isolation of public endpoints and admin/control endpoints; abuse controls
API
Authentication/keys for premium services; rate limiting; request validation
Oracle
Source validation, freshness checks, outlier handling, circuit breakers
Risk
Deterministic calculations, versioned formulas, test vectors
Contracts
Independent audits before mainnet capital exposure
Vaults
Pause controls, caps, withdrawal rules, strategy isolation
Liquidations
Permissionless verification where possible; replay/idempotency protection
Keys
Hardware/isolated signing, multisig for privileged operations
Infrastructure
TLS, secrets management, backups, monitoring, least privilege
Operations
Incident runbooks, alerting, rollback/disable procedures


No capital product should move to production merely because the smart contract compiles and passes unit tests.
12.1 Controller Security
Run public read routes separately from privileged control routes where practical.
Do not expose oracle signing credentials to the public API process; protect oracle secrets and signing identities outside application source/config.
Restrict price commit and circuit-breaker reset operations with authentication/authorization.
Validate every price before publication against sanity bounds and source confidence.
Monitor source disagreement and publication anomalies.
Use least-privilege credentials for Horizon, database and oracle operations.
12.2 Oracle Security
Publisher authority should be narrowly scoped (multisig/controlled publisher where feasible).
Price methodology must be documented and versioned.
Stale data must be detectable.
Circuit-breaker behavior must be deterministic and tested.
Emergency disable/recovery procedures must be documented.
Consumers should have explicit behavior for missing/degraded oracle states.
12.3 Capital Security
Strategy contracts should be isolated where possible.
Use deposit/position caps.
Include pause/emergency controls.
Separate strategy logic from vault accounting.
Audit contracts before meaningful mainnet TVL.
Stage deployment with small limits before unrestricted growth.
12.4 Operational Security
Separate mainnet and testnet credentials and databases.
Use secret management rather than committed environment secrets.
Restrict administrative ports to private networks.
Record security-sensitive actions in an audit log.
Maintain incident-response and key-rotation procedures.
13. Reliability & Operations
Mainnet/testnet separation
Independent RPC/Horizon health checks
Prometheus/Grafana or equivalent metrics
Latency and error-rate monitoring
Database backups
Replay/re-index procedures
Rate limiting and abuse detection
API versioning
Status page / incident communication
SLOs for public infrastructure
The existing RPC repository already contains monitoring infrastructure and separates an internal admin endpoint from the public JSON-RPC endpoint; the production architecture should preserve and formalize that separation.
13.1 Suggested Service SLOs
RPC availability and p95 latency.
Horizon/API availability and p95 latency.
Controller availability.
Price freshness age.
Oracle publication success rate and publication lag.
Indexer lag.
Webhook delivery success rate.
13.2 Operational Dashboards
Infrastructure health dashboard.
RPC request/latency/error dashboard.
Horizon/API traffic dashboard.
Controller source-health dashboard.
Oracle confidence and circuit-breaker dashboard.
Indexer lag/completeness dashboard.
Financial-data quality dashboard.
13.3 Recovery
Every stateful service should have a documented recovery path: restore database → identify last checkpoint → replay source data → reconcile derived state → reopen public traffic. Oracle and capital services additionally require a controlled safe-mode procedure.
14. Revenue Model
Revenue should be layered rather than dependent on one speculative token or one DeFi application.
Revenue line
Mechanism
When
RPC/API
Usage tiers, rate limits, premium access
Early
Data
Premium historical/financial data
After data layer matures
Oracle
Protocol/API access fees
After reliability is demonstrated
Risk
Risk API / protocol integration fees
After risk engine exists
Execution
Routing/execution service fee
When liquidity supports it
Capital
Vault management/performance fees where appropriate
After validated strategy
Liquidation
Infrastructure/execution fee
After lending markets exist
Enterprise
Dedicated infrastructure / SLA / private capacity
As ecosystem matures


Open-source components can remain publicly auditable while managed infrastructure, reliability, premium data, premium capacity and protocol services create commercial value.
14.1 Infrastructure Revenue
RPC usage tiers.
Higher-rate API access.
Dedicated RPC/API capacity.
Enterprise or protocol SLAs.
Historical data access.
Premium analytics.
14.2 Financial Infrastructure Revenue
Oracle-as-a-service for protocols that need managed feeds.
Risk APIs.
Financial-data subscriptions (historical market data).
Simulation/quote services.
14.3 Protocol Revenue
Execution/routing fees when Suban routes meaningful volume.
Vault management fees.
Performance fees where economically justified and transparently disclosed.
Liquidation infrastructure/execution fees.
The commercial moat should come from infrastructure reliability, data quality, integrations and operational trust — not from forcing a speculative token into every product.
15. Product Validation Framework
Every proposed product must pass the following test before significant capital is committed.
Question
Evidence required
Who uses it?
Identifiable user/developer group
What problem exists?
Observed workflow or measurable pain
What do they use today?
Existing alternatives
Why Pi?
Pi-specific reason the product is needed
Why Suban?
Infrastructure or distribution advantage
What generates revenue?
Concrete fee or subscription mechanism
What is the dependency?
Liquidity, oracle, contracts, users, etc.
What can kill it?
Explicit failure conditions
What is the smallest test?
Prototype with measurable success criteria that could falsify the idea


This framework is deliberately designed to prevent founder-generated ideas from being mistaken for market validation. Aquarius/Blend remain design references and candidate implementations, not automatic roadmap commitments; lending and liquidity products should be selected from measured demand and infrastructure conditions.
15.1 Validation Scorecard
Problem severity: Is the problem expensive, frequent or blocking?
Existing workaround: What are users/developers doing today?
Pi specificity: Why does Pi create or amplify the problem?
Distribution: How will the first users find the product?
Technical advantage: Why is Suban structurally better positioned?
Economic model: Who pays and why?
Defensibility: What becomes harder to replace after adoption?
Security burden: What new capital or trust risk does the product introduce?
Falsification test: What result would cause us to stop building it?
15.2 Infrastructure vs Capital Decision Rule
If a capability is broadly reusable, low-capital and useful to several applications, Suban may build it ahead of demand. If a capability requires user deposits, token incentives, substantial liquidity or high smart-contract risk, Suban should first prove demand with analytics, testnet or non-custodial infrastructure.
16. 2026–2027 Roadmap
Q4 2026 — Consolidate (Controller + Core)
Create canonical Suban architecture documentation.
Inventory all existing services/contracts.
Map repo ownership and deployment boundaries.
Treat suban-controller as a first-class production service; document controller routes and privileged controls.
Document source weights, freshness policy, confidence calculation and circuit-breaker behavior.
Separate public read traffic from oracle/control traffic.
Add source-health, oracle-push and circuit-breaker observability.
Map exact deployment topology across oracle.suban.org, horizon.suban.org, testnet.suban.org, rpc.suban.org and testrpc.suban.org.
Standardize API versioning.
Harden RPC/Horizon production operations.
Document existing price oracle.
Document escrow and contract discovery.
Define metrics/SLOs.
Resolve or formally document the getLedgerEntries contract-state limitation.
Exit criterion: a new developer can understand what Suban is, what service to use, how the repositories relate, and how to safely integrate core services without reading source code.
Q1 2027 — Data Platform
Unified/canonical indexed data model.
Event indexing, with an ingestion/checkpoint/replay pipeline.
Contract metadata/discovery improvements.
Webhooks.
Historical query capabilities.
Developer API keys, usage metering and usage metrics.
Define data freshness and completeness guarantees.
Exit criterion: external developers can build explorers, analytics and applications without maintaining their own Pi data ingestion pipeline.
Q2 2027 — Financial Infrastructure
Financial asset/pool data.
Historical market data; persist historical price observations.
Extend existing price oracle with freshness/provenance; expose source provenance and confidence.
TWAP where data quality permits.
Liquidity, volume and volatility metrics where on-chain data supports them.
Initial risk engine.
Publish standardized Financial Data APIs.
Exit criterion: at least one external or internal financial application consumes Suban financial data/oracle services.
Q3 2027 — Execution & Capital Experiments
Quote/simulation service and interfaces.
Route discovery prototype — only across real liquidity sources.
Liquidity analytics.
Liquidity optimization prototype (analytics first).
Vault architecture on testnet (testnet vault framework).
Capital strategy experiments; measure strategy economics before mainnet capital.
Exit criterion: execution and capital prototypes demonstrate measurable usage or economic activity before mainnet capital deployment — measured economic activity must justify one capital/execution product.
Q4 2027 — Choose the Winning Financial Product
Only after observing real ecosystem activity should the team select a major capital product, choosing from evidence: router, vault, lending, liquidation network or settlement.
Candidate
Trigger to build
Liquidity Vault
Meaningful pool activity + identifiable LP demand + sustainable fee source
Lending
Borrow demand + collateral liquidity + liquidation capacity
Router
Multiple meaningful liquidity sources
Liquidation Network
Multiple lending markets or clear external demand
Settlement
Multiple financial applications relying on shared financial state


A router wins if liquidity is fragmented and route volume is real.
A vault wins if LP demand and fee economics are real.
Lending wins only if borrow demand, collateral liquidity and liquidation capacity exist.
A liquidation network wins if multiple lending markets need the same infrastructure.
Settlement wins if multiple financial applications need common financial-state reconciliation.
Exit criterion: one product has repeat users, defensible economics and a security path.
The roadmap intentionally does not preselect Aquarius or Blend as the winner.
17. Milestones & Exit Criteria
Milestone
Success metric
Core reliability
Measured uptime, latency, error-rate and recovery targets established for RPC, API and controller
Developer platform
External developer successfully integrates without direct maintainer intervention
Data platform
Indexed queries materially reduce need for direct node/Horizon integration
Financial platform
Financial API consumed by at least one real application
Oracle
Documented and observable freshness, source provenance, confidence and failure behavior
Execution
Quotes/simulations demonstrably reduce integration complexity and produce reproducible results
Capital
A strategy has real demand and a defensible, clearly identified source of return — no unsustainable yield assumptions
Network
Third-party protocols consume Suban services in production/testnet


17.1 Core Infrastructure Milestone
Done when RPC/API/controller are independently observable, production boundaries are documented, failure recovery is tested and public/admin surfaces are separated.
17.2 Data Platform Milestone
Done when an external developer can obtain normalized Pi data, historical records and event notifications without building a custom ingestion pipeline.
17.3 Financial Platform Milestone
Done when at least one real application consumes Suban financial data and can inspect price freshness/confidence rather than receiving an unexplained number.
17.4 Execution Milestone
Done when quote/simulation/route services demonstrably reduce integration work and produce reproducible execution estimates.
17.5 Capital Milestone
Done only when a strategy has identified demand, a defensible source of return, risk limits, contract security review and a staged deployment plan.
18. Dependencies
Pi network availability and protocol capabilities
Accessible blockchain/Horizon data
Smart-contract execution and stable interfaces
Sufficient ecosystem liquidity for financial products
Third-party application adoption
Security/audit capacity
Reliable node and infrastructure operations
Market demand
The last dependency is the most important. Technical feasibility does not imply product viability.
19. Risks
Risk / dependency
Mitigation
Building unused products / product-market mismatch (a technically strong product may have no usage)
Stage-gated validation; mandatory validation framework and measurable exit criteria
RPC/API competition
Differentiate through integrated data + financial services + reliability
Pi protocol/API evolution (RPC/Horizon compatibility can break)
Compatibility matrix, CI/scheduled checks, versioned adapters
Controller external-source dependence (price sources can fail or disagree)
Minimum-source rule, staleness checks, outlier filtering, circuit breaker
Oracle manipulation
Multiple sources, TWAP, liquidity filters, circuit breakers
Oracle key compromise (incorrect on-chain price publication)
Secret isolation, restricted admin access, multisig/controlled publisher where feasible
Contract-state indexing gap (some application state cannot be queried through the current getLedgerEntries path)
Build supported indexers/contract event ingestion rather than depending on the failing method
Smart-contract bugs (capital loss)
Audits, staged testnet/mainnet rollout, caps, isolated strategies and emergency controls
Low liquidity (router/vault/lending economics may be weak)
Do not launch capital products before measurable liquidity
Low borrowing demand
Treat lending as an optional later product
Operational outage
Redundancy, monitoring, backups and runbooks
Repository fragmentation
Strict ownership map and service boundaries
Founder bias
Formal product validation framework


20. Non-Goals
Suban will not automatically build every DeFi primitive.
Suban will not assume lending is the default financial product.
Suban will not create a token merely to manufacture yield.
Suban will not duplicate Pi-native infrastructure without a clear reliability, developer-experience or economic advantage.
Suban will not treat Aquarius or Blend as validated products.
Suban will not expose internal admin/control APIs — including the controller’s price-commit, circuit-breaker reset and oracle-publication paths — as unrestricted public APIs.
Suban will not deploy public capital strategies before security and economic validation.
21. Definition of Done
A Suban module is production-ready only when:
The responsibility and API boundary are documented.
The repository owner is defined (the owning repository/module is identified).
Failure modes are documented.
Metrics and alerts exist.
Security assumptions are explicit.
Integration tests cover critical flows.
Versioning/compatibility policy exists.
Operational runbook exists.
Revenue mechanism is understood if the service is commercial.
For capital-bearing contracts: independent audit/security review and staged deployment requirements are satisfied.
22. Immediate Next Actions
Documentation and ownership
Create /docs/ARCHITECTURE.md as the canonical technical architecture.
Create /docs/ROADMAP.md from this document with GitHub milestones/issues.
Create /docs/PRODUCTS.md separating existing products from candidate products.
Create /docs/API.md documenting current RPC/API endpoints and planned namespaces.
Create /docs/SECURITY.md covering RPC, API, oracle and contract security.
Create a repository ownership matrix for Suban, suban-rpc, Suban-api and suban-controller.
Controller and oracle
Promote suban-controller to a first-class component in the Suban architecture diagram and deployment inventory.
Split controller public-read routes from privileged price commit / circuit-breaker / oracle-push controls.
Document the four current price sources, weights, freshness thresholds, confidence calculation and circuit-breaker policy.
Verify the on-chain oracle contract interface and publisher authority used by OraclePushService before treating it as production-grade.
Inventory the existing price oracle and document its current data sources, update mechanism, failure behavior and consumers.
Define a common API contract between suban-controller, Suban-api and suban-rpc to avoid overlapping/ambiguous data surfaces.
Instrument production usage — request volume, source failures, aggregation confidence, circuit-breaker events and oracle push success/failure — so roadmap decisions are based on actual traffic, not assumptions.
Data platform and capital gating
Design the first indexed data model around events/accounts/transactions that are actually available through supported Pi interfaces.
Keep getLedgerEntries as a documented compatibility limitation until a tested build supports the required contract-state key format.
Build the Data Platform expansion before committing to a major lending/vault launch.
After Q1/Q2 2027 data exists, reassess liquidity, lending, routing and capital opportunities using measured evidence.
Do not start a mainnet lending/vault deployment until the data, oracle, liquidation and security prerequisites are demonstrably satisfied.
Appendix A — Current Repository Evidence
The public Suban repository currently contains contracts, crates, docs, frontend/escrow-viewer, SDK and services.
The public suban-rpc repository describes the system as a JSON-RPC server for Pi Network and lists network queries, transaction submission, transaction status/history, preflight simulation and smart-contract interaction. It also contains monitoring, a Pi DApp suite, scripts and smart-contract assets.
The public Suban-api repository describes itself as a Horizon API wrapper with caching, rate limiting, enhanced endpoints and a full Horizon proxy, including network summary, accounts, transactions, ledgers and assets.
The public organization currently includes Suban and Suban-api among its repositories, alongside other Pi/DeFi projects.
The suban-controller repository is now public and has been incorporated into this revision as a first-class Suban component. Its published implementation establishes the controller as an off-chain TypeScript/Node.js data/oracle control plane with multi-source price aggregation, chain analytics, Horizon proxying and optional on-chain oracle publication. This replaces the earlier note that the controller’s public URL could not be retrieved and its implementation had to be verified.
A.1 suban-controller Source-Level Map
src/index.ts — application composition/startup; wires price aggregation (four sources), routes, chain-data services and oracle-push behavior.
src/services/aggregator.ts — aggregation logic including source collection, staleness checks, minimum sources, outlier handling, weighted aggregation, confidence and deviation/circuit-breaker behavior.
src/services/oracle-push.ts — on-chain oracle publication workflow, price scaling, confidence publication, Soroban invocation and push statistics.
src/routes/api.ts — price, source, health, circuit-breaker and deviation-alert API routes.
src/routes/chaindata.ts — chain statistics, ledger, account and transaction routes.
package.json — TypeScript/Node package definition, runtime dependencies and build/test scripts.
A.2 Controller Production Checklist
External source credentials/configuration isolated.
Source timeouts and retry policy configured.
Minimum source count configured.
Staleness threshold documented.
Outlier threshold documented.
Aggregation weights versioned.
Confidence calculation documented.
Circuit-breaker threshold documented.
Public/private route separation enforced.
Oracle publisher authority restricted.
Oracle push result monitored.
Controller logs centralized.
Health checks exposed.
Restart/recovery tested.
Source API changes tested before production rollout.
A.3 Source Evidence & Notes
The controller facts in this document are grounded in the now-public suban-controller repository (https://github.com/Pi-Defi-world/suban-controller) and the existing Suban repositories previously reviewed.
Source
Evidence used
suban-controller README
Controller purpose, four price sources, weighted aggregation, analytics, Horizon proxy, endpoints and configuration
suban-controller src/index.ts
Express composition, aggregator initialization, four sources, oracle-push service, route boundaries and service startup
suban-controller src/services/aggregator.ts
Staleness checks, minimum sources, outlier filtering, weighted average, confidence score, deviation alerts and circuit breaker
suban-controller src/services/oracle-push.ts
On-chain oracle publication, price scaling, confidence publication, Soroban invocation and push statistics
suban-controller src/routes/api.ts
Price, source, health, circuit-breaker and deviation-alert endpoints
suban-controller src/routes/chaindata.ts
Chain stats, ledger, account and transaction endpoints
suban-controller package.json
TypeScript/Node runtime, dependencies, scripts and package identity
Suban / suban-rpc / Suban-api repositories
Existing platform, RPC and Horizon/API boundaries


Repository facts describe the code as currently published. Strategic roadmap items are architecture recommendations and should be validated against deployment behavior, production metrics and real ecosystem demand before implementation.
Appendix B — Terminology
Term
Definition
Core
Existing blockchain connectivity and reusable protocol foundation
Data
Normalized/indexed blockchain information
Financial Data
Market/pool/position information derived from blockchain state
Oracle
Price/value publication service; Suban already has a price oracle
Risk Engine
Derived risk calculations using price, liquidity and state data
Execution
Quote, route, simulate, construct and execute financial transactions
Capital
Vaults and strategy management
Protection
Liquidation, monitoring and settlement services
Strategy
A defined capital-allocation algorithm with explicit sources of return and risk


Appendix C — Strategic One-Liner
Suban is the infrastructure layer that lets Pi applications read, understand, execute and eventually manage financial activity without rebuilding the underlying infrastructure themselves.
The 2026–2027 objective is not to guess which DeFi protocol wins. It is to make Suban indispensable to the applications that emerge — and then operate the financial products that the evidence says are worth operating.
Appendix D — Canonical Architecture Flow
Pi applications
       ↓
Suban API / SDK
       ↓
┌──────────────────────────────────────────────────────────────┐
│ Suban Data Plane                                             │
│ Horizon/API + Controller + future indexer/event pipeline     │
└──────────────────────────────────────────────────────────────┘
       ↓
Financial Data / Oracle Extensions / Risk
       ↓
Execution: RPC + simulation + routing
       ↓
Capital / Protection: vaults, lending, liquidation, settlement
       ↓
Pi smart contracts / native DEX / ecosystem applications
