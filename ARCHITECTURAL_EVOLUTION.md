# DCAP Architectural Evolution: A First-Principles Redesign

## Executive Summary

DCAP (Decentralized Commerce Agentic Protocol) is an MVP-grade Rust system aspiring to enable autonomous LLM-to-LLM commerce. It contains the *shape* of a protocol but lacks the structural integrity to evolve into one. This document presents a forced architectural evolution: moving from a tightly-coupled, mock-heavy web-service assembly to a protocol-kernel-centric, event-sourced, port/adapter architecture that can compound in capability over time.

---

## Phase 0: System Reconstruction

### What the System Claims to Be

Per `README.md` and `Cargo.toml`, DCAP is:
- A **decentralized commerce protocol** for LLM-to-LLM negotiation
- A **multi-rail settlement layer** (Stripe, Solana, Escrow)
- A **trust/reputation system** with JWT-based auth
- An **MCP-compliant** server for standardized agent communication
- A **data flywheel** generating training data from negotiations

### What the System Actually Is

After direct inspection of ~4,000 LOC:

1. **A set of Axum binaries with hardcoded mock data.** The `seller_agent.rs` binary returns JSON literals for products. The `discovery.rs` binary's `DiscoveryServer` returns empty vectors. The `mcp_server.rs` serves mock product catalogs from hardcoded structs.

2. **An orphaned database layer.** `database.rs` contains a complete SQLite schema with migrations, indices, and CRUD operations. **None of the binaries use it.** `DiscoveryServer` has the database field commented out. `TrustSystem` uses an in-memory `HashMap`. `BuyerAgent` stores negotiations in a `HashMap`.

3. **A broken MCP implementation.** The "MCP server" opens a raw TCP socket, reads 1024 bytes into a fixed buffer, parses custom JSON, and responds. It does not implement the Model Context Protocol (stdio or SSE transport, JSON-RPC 2.0 framing, capability negotiation, lifecycle management). It is a custom TCP protocol wearing MCP's name.

4. **No actual LLM integration.** Despite the `async-openai` dependency and LLM config structs, no business logic calls OpenAI. Pricing "factors" are hardcoded multipliers (`if quantity > 10 { 0.95 }`).

5. **No cryptography.** `ed25519-dalek` is in `Cargo.toml`. The `generate_public_key()` function returns `"mock_public_key_base64_encoded"`.

6. **Centralized trust in a decentralized protocol.** The trust system requires a single `JWT_SECRET` env var and stores reputation in a local `HashMap`. There is no federation, no consensus, no Sybil resistance beyond wishful thinking.

### Implicit Assumptions Surfaced

| Assumption | Evidence | Risk |
|------------|----------|------|
| All agents trust the same discovery server | `DiscoveryService::new(endpoint: String)` | Single point of failure/censorship |
| HTTP request/response is sufficient for negotiation | All agent interaction via `reqwest` POST/GET | No async events, no saga pattern, no recovery |
| Buyer and Seller are the only agent roles | `AgentType` enum has exactly two variants | Cannot model brokers, arbitrageurs, insurers, oracles |
| Reputation is a scalar 0-100 | `reputation_score: u32` | Loses all nuance (recency, category expertise, dispute history) |
| Settlement happens synchronously | `accept_quote` calls `settlement.create_payment` inline | No escrow state machine, no partial failure handling |
| One quote per negotiation | `quote_id: Option<TransactionId>` | Cannot model multi-seller RFQs, competitive bidding |

### The Design Philosophy Gap

The README advocates for an **"anti-walled garden"** — a decentralized, protocol-first commerce mesh. But the implementation is a **centralized client-server system** with five separate binaries that must be manually orchestrated. The architecture contradicts the mission.

A protocol is not a set of services. A protocol is a **grammar of messages** that independent implementations can speak. DCAP has no grammar — it has REST endpoints returning mock JSON.

---

## Phase 1: Deep Structural Diagnosis

### Root-Level Limitations (Not Symptoms)

#### 1.1 The Missing Kernel

There is no **irreducible core** that defines what DCAP *is*. The `lib.rs` re-exports modules without cohesion. A healthy protocol has a kernel: the minimal set of types and invariants that all participants must agree on. DCAP's kernel is implicit, scattered across `model.rs`, `agent.rs`, and the binaries.

**Symptom:** Changes to `Product` require touching model, database schema, seller agent, buyer agent, MCP resources, and discovery.  
**Root cause:** No bounded context. No kernel.

#### 1.2 Stateful Services Without State Management

`BuyerAgent` holds `active_negotiations: HashMap<TransactionId, Negotiation>`. `TrustSystem` holds `reputation_cache: HashMap<AgentId, ReputationScore>`. These are in-process, non-replicated, non-persistent state.

**Failure mode under scale:** Restart the buyer-agent binary → all negotiations vanish. Run two instances behind a load balancer → split brain on negotiation state.

#### 1.3 Synchronous Distributed Transactions

The `accept_quote` method performs a distributed transaction across three systems:
1. Updates local negotiation state
2. Calls `settlement.create_payment`
3. Calls `trust.update_reputation` (twice)

There is no saga, no outbox, no compensating transaction. If settlement succeeds but the process crashes before trust update, the ledger and reputation diverge.

#### 1.4 The Enum Extensibility Trap

`PaymentMethod` is an enum:
```rust
pub enum PaymentMethod { Stripe, Solana, Escrow }
```
Adding a payment method requires modifying the enum, recompiling the library, and updating every `match` expression. In a protocol, payment methods should be **open** (dynamic registration via capability negotiation).

Same problem with `AgentType`, `NegotiationStatus`, `MessageType`, `TrustLevel`.

#### 1.5 Error Handling as a God Object

`NegotiationError` is a single enum for the entire system:
- Database errors
- Network errors
- Auth errors
- Payment errors
- Validation errors

This creates **abstraction leaks:** `BuyerAgent` returns `NegotiationError::Database` even though it doesn't own a database. The `#[from] sqlx::Error` conversion means any SQL error anywhere becomes a negotiation error, destroying error context.

#### 1.6 No Backpressure or Resilience

The buyer agent spawns HTTP requests to sellers without:
- Connection pooling configuration
- Timeout configuration (uses `reqwest` defaults)
- Circuit breakers
- Rate limiting
- Retry with jitter

Under load, this cascades. The `SellerAgent` has no request concurrency limits.

#### 1.7 The Discovery Mirage

Discovery claims to enable decentralized agent finding. In reality:
- Sellers must know the discovery endpoint at boot
- Buyers must know the discovery endpoint at boot
- The discovery server is a single SQLite-backed Axum app
- There is no gossip, no DHT, no federation
- Product search by category is literally unimplemented (`// For now, we'll just return all sellers`)

This is not discovery. This is a **centralized registry**.

### Failure Mode Map

| Scenario | Current Behavior | Desired Behavior |
|----------|-----------------|------------------|
| Discovery server restarts | All agent registrations lost (in-memory) | Agents re-announce via heartbeat; gossip mesh maintains visibility |
| Seller agent restarts mid-negotiation | Negotiation state lost | Negotiation is event-sourced; seller replays from stream |
| Settlement webhook delayed | Trust not updated; state inconsistent | Outbox pattern guarantees eventual consistency |
| Malicious buyer spams RFQs | No rate limiting; seller CPU wasted | Reputation-weighted rate limits; proof-of-work or stake gating |
| Compromised JWT secret | Attacker can impersonate any agent | Ed25519 pairwise verification; no shared secrets |
| Need to add auction mechanism | Rewrite `Negotiation` struct and all match arms | Plugin architecture: new interaction pattern registers dynamically |

---

## Phase 2: First-Principles Reframing

### What Is Autonomous Commerce, Really?

Strip away the LLM marketing. At its core, autonomous commerce is:

> **Asynchronous commitment exchange between autonomous principals.**

A buyer commits to a need (RFQ). A seller commits to a capability (quote). They iteratively refine commitments (negotiation). They atomically exchange value for obligation (settlement). Third parties attest to history (trust).

### Core Invariants

1. **Negotiation is a deterministic state machine.** Given the same event stream, any observer must arrive at the same state.
2. **Commitments are time-bounded.** Every quote, offer, and escrow has a TTL. Time is a first-class domain concept.
3. **Identity is self-sovereign.** An agent proves identity via keypair, not shared secret.
4. **Settlement is atomic relative to negotiation.** A settled negotiation must have a corresponding verifiable payment intent or escrow lock.
5. **Trust is a derived view, not source truth.** Reputation is computed from observable transaction history, not stored as a scalar.

### Clean Abstractions

#### The Protocol Kernel

The kernel contains only:
- `Identity` (Ed25519 public key)
- `Commitment` (an offer or RFQ with TTL and signature)
- `Transition` (a domain event: `Quoted`, `Countered`, `Accepted`, `Settled`, `Disputed`)
- `NegotiationState` (fold of transitions)

Everything else is an adapter.

#### The Port/Adapter Boundary

| Port | Responsibility | Implementations |
|------|---------------|-----------------|
| `Discovery` | Resolve `Identity` → `Endpoint` / `Capability` | Centralized registry, DHT, DNS, static file |
| `Settlement` | Convert `Acceptance` → `PaymentIntent` | Stripe, Solana, Escrow contract, Mock |
| `TrustEngine` | Compute `TrustVector` from `TransactionLog` | Local graph, federated attestation, on-chain |
| `Persistence` | Append and query `Event`s | SQLite, PostgreSQL, event log, memory |
| `Transport` | Deliver `Envelope` to `Endpoint` | HTTP, WebSocket, libp2p, MCP stdio |
| `Oracle` | Provide external truth | Price feeds, shipping trackers, KYC |

#### Minimal, Composable Interfaces

```rust
// Kernel trait: ~10 methods total
pub trait NegotiationKernel {
    type Error;
    fn apply(&self, state: &State, event: Event) -> Result<State, Self::Error>;
    fn validate(&self, event: &Event) -> Result<(), Self::Error>;
}

// Port trait: ~3 methods
pub trait Discovery {
    type Error;
    async fn announce(&self, record: AgentRecord) -> Result<(), Self::Error>;
    async fn resolve(&self, id: &Identity) -> Result<AgentRecord, Self::Error>;
    async fn search(&self, query: &Query) -> Result<Vec<AgentRecord>, Self::Error>;
}
```

### Extensibility and Adaptability

The system must grow without structural changes:
- New interaction patterns (auctions, reverse auctions, subscriptions) = new `Event` variants via plugin registry, not enum extension.
- New settlement rails = new `Settlement` adapter, no kernel changes.
- New trust models = new `TrustEngine` implementation, no protocol changes.
- New transport = new `Transport` adapter.

---

## Phase 3: Radical Redesign

### 3.1 New Module Architecture

```
dcap/
├── kernel/
│   ├── lib.rs          # Re-exports only
│   ├── identity.rs     # Ed25519 wrapper, DID-style self-sovereign IDs
│   ├── commitment.rs   # RFQ, Quote, CounterOffer as signed, time-bounded commitments
│   ├── state_machine.rs # Deterministic negotiation lifecycle
│   ├── event.rs        # Domain events: the protocol grammar
│   └── validation.rs   # Invariant checks, TTL verification, signature validation
├── ports/
│   ├── mod.rs
│   ├── discovery.rs    # Trait + types
│   ├── settlement.rs   # Trait + types
│   ├── trust.rs        # Trait + types
│   ├── persistence.rs  # Trait + types (event store)
│   └── transport.rs    # Trait + types
├── adapters/
│   ├── discovery/
│   │   ├── registry.rs     # HTTP centralized registry (current behavior, extracted)
│   │   └── static_file.rs  # File-based discovery for testing
│   ├── settlement/
│   │   ├── mock.rs
│   │   ├── stripe.rs       # Future: real integration
│   │   └── solana.rs       # Future: real integration
│   ├── trust/
│   │   ├── local_graph.rs  # SQLite-backed trust graph
│   │   └── null.rs         # No-op for testing
│   ├── persistence/
│   │   ├── sqlite.rs       # Event store on SQLite
│   │   └── memory.rs       # In-memory for testing
│   └── transport/
│       ├── http.rs         # Axum client/server
│       └── mcp_stdio.rs    # Real MCP over stdio (replace broken TCP)
├── protocol/
│   ├── mod.rs
│   ├── envelope.rs     # Signed, routed message container
│   ├── codec.rs        # Serialization (JSON, CBOR, future: protobuf)
│   └── mcp_mapping.rs  # Map DCAP events to MCP tools/resources/prompts
├── agent/
│   ├── mod.rs
│   ├── runtime.rs      # Actor-like mailbox + event loop
│   ├── buyer.rs        # Buyer-specific behavior (strategy plugin)
│   ├── seller.rs       # Seller-specific behavior (strategy plugin)
│   └── strategy.rs     # Trait for LLM/hardcoded/learning strategies
├── app/
│   └── bin/...         # Thin binaries that wire adapters
└── lib.rs              # Only re-exports kernel + ports
```

### 3.2 Event-Sourced Negotiation State Machine

The current `Negotiation` struct is mutable state with ad-hoc transitions:
```rust
pub fn accept(&mut self, final_price: f64) -> Result<()> { ... }
```

**Redesign:** Negotiation is a pure function over events.

```rust
// events are immutable facts
pub enum NegotiationEvent {
    RfqSubmitted { rfq: RFQ, signature: Signature },
    Quoted { quote: Quote, signature: Signature },
    Countered { offer: Offer, signature: Signature },
    Accepted { price: Decimal, signature: Signature },
    Rejected { reason: Option<String>, signature: Signature },
    Settled { payment_intent: PaymentIntentId, signature: Signature },
    Expired { at: Timestamp },
    Disputed { reason: String, signature: Signature },
}

// state is a derived fold
pub struct NegotiationState {
    pub id: NegotiationId,
    pub rfq: RFQ,
    pub quotes: Vec<Quote>,
    pub current_offer: Option<Offer>,
    pub status: Status,
    pub participants: BTreeMap<Identity, ParticipantState>,
    pub expires_at: Timestamp,
}

impl NegotiationState {
    pub fn apply(mut self, event: &NegotiationEvent) -> Result<Self, TransitionError> {
        match (self.status, event) {
            (Status::Pending, NegotiationEvent::Quoted { .. }) => { ... }
            (Status::Quoted, NegotiationEvent::Countered { .. }) => { ... }
            (Status::Quoted | Status::Negotiating, NegotiationEvent::Accepted { .. }) => { ... }
            // Invalid transitions are compile-time impossible by pattern match exhaustiveness
            _ => Err(TransitionError::Invalid { from: self.status, event: event.kind() }),
        }
    }
}
```

**Why this is strictly better:**
1. **Auditability:** Every state change is a permanent, signed, timestamped event.
2. **Reproducibility:** Any participant can reconstruct state by replaying events.
3. **Concurrency:** Conflicts become explicit (two `Accepted` events = detectable double-spend).
4. **Testing:** State machine is pure, sync, no mocks needed.
5. **Distribution:** Events are the replication unit. State can be rebuilt on any node.

### 3.3 Identity and Cryptography

Replace UUID + mock public key with self-sovereign identity:

```rust
pub struct Identity {
    pub did: String,           // "did:dcap:<base58-of-pubkey>"
    pub verifying_key: ed25519_dalek::VerifyingKey,
}

pub struct Envelope {
    pub sender: Identity,
    pub recipient: Identity,
    pub payload: Payload,
    pub timestamp: Timestamp,
    pub nonce: Nonce,
    pub signature: Signature,
}

impl Envelope {
    pub fn verify(&self) -> Result<(), CryptoError> { ... }
}
```

No JWT. No shared secrets. Agents authenticate every message. Discovery servers are untrusted directories, not certificate authorities.

### 3.4 Trust as a Graph, Not a Scalar

Replace `reputation_score: u32` with a multi-dimensional trust vector:

```rust
pub struct TrustVector {
    pub reliability: f64,        // % of commitments honored
    pub responsiveness: f64,     // response time percentile
    pub dispute_rate: f64,       // disputes / total transactions
    pub volume_weight: f64,      // log(total value settled)
    pub category_expertise: HashMap<Category, f64>,
    pub recency_decay: f64,      // time-weighted (recent matters more)
}

pub trait TrustEngine {
    async fn attest(&self, tx: &TransactionRecord) -> Result<Attestation, Error>;
    async fn query(&self, id: &Identity, context: &TrustContext) -> Result<TrustVector, Error>;
}
```

Attestations are signed statements about observed transactions. A `TrustEngine` implementation can be:
- **Local:** My node's observed history
- **Federated:** Weighted aggregate of attestations from peers I trust
- **On-chain:** Solana program storing attestations

This enables **subjective trust.** I may trust a seller for electronics but not for real estate, based on category-specific history.

### 3.5 The Outbox and Saga Pattern for Settlement

Current flow (synchronous, fragile):
```
accept_quote → update state → create_payment → update_trust → done
```

New flow (async, reliable):
```
1. Buyer emits NegotiationEvent::Accepted
2. Kernel transitions state to Accepted
3. Outbox observer writes: [CreatePayment, UpdateTrust]
4. Settlement adapter polls outbox, processes CreatePayment
5. On success, Settlement emits PaymentEvent::IntentCreated
6. Kernel observes payment event, transitions to Settled
7. Trust adapter computes new TrustVector from confirmed TransactionRecord
```

The outbox table (in SQLite/Postgres) guarantees exactly-once processing:
```sql
CREATE TABLE outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL,
    payload BLOB NOT NULL,
    headers BLOB,
    created_at DATETIME NOT NULL,
    processed_at DATETIME,
    retry_count INTEGER DEFAULT 0,
    UNIQUE NULLS NOT DISTINCT (topic, payload) -- idempotency key
);
```

### 3.6 Real MCP Integration

Remove the broken custom TCP server. Implement MCP properly:

```rust
// dcap-adapters/src/transport/mcp_stdio.rs
pub struct McpTransport {
    stdin: tokio::io::Stdin,
    stdout: tokio::io::Stdout,
}

impl Transport for McpTransport {
    async fn recv(&mut self) -> Result<Envelope, Error> {
        // Read JSON-RPC 2.0 message from stdin
        // Parse as MCP request
        // Map to DCAP Envelope
    }
    async fn send(&mut self, envelope: &Envelope) -> Result<(), Error> {
        // Map DCAP Envelope to MCP response/notification
        // Write JSON-RPC 2.0 message to stdout
    }
}
```

DCAP exposes to MCP as:
- **Tools:** `dcap_submit_rfq`, `dcap_submit_quote`, `dcap_accept`, `dcap_search_agents`
- **Resources:** `dcap://negotiations/{id}`, `dcap://agents/{did}/trust`
- **Prompts:** `dcap_negotiation_strategy`, `dcap_trust_assessment`

This makes DCAP usable from Claude Desktop, Cursor, or any MCP client without custom networking.

### 3.7 Agent Runtime: From Struct to Actor

Current `BuyerAgent` is a struct with methods. It cannot handle concurrent negotiations cleanly, has no mailbox, and crashes lose state.

New design:
```rust
pub struct AgentRuntime {
    identity: Identity,
    kernel: Arc<NegotiationKernel>,
    persistence: Arc<dyn Persistence>,
    transport: Arc<dyn Transport>,
    strategy: Arc<dyn Strategy>,
    mailbox: mpsc::Channel<AgentCommand>,
}

enum AgentCommand {
    Receive(Envelope),
    Submit(RFQ),
    QueryState(NegotiationId, oneshot::Sender<NegotiationState>),
    Shutdown,
}

impl AgentRuntime {
    async fn run(mut self) {
        while let Some(cmd) = self.mailbox.recv().await {
            match cmd {
                AgentCommand::Receive(envelope) => self.handle_envelope(envelope).await,
                AgentCommand::Submit(rfq) => self.initiate_negotiation(rfq).await,
                // ...
            }
        }
    }
}
```

The runtime:
1. Receives messages via transport
2. Validates signatures via kernel
3. Applies events via kernel
4. Persists events
5. Consults strategy for responses
6. Sends responses via transport

State is never in the runtime. It is always rebuilt from the event store.

---

## Phase 4: Adversarial Self-Critique

### Where Is It Overengineered?

**The event-sourcing layer could be overkill for an MVP.** If the goal is "get 10 Shopify stores using this in a month," SQLite + CRUD is faster to build. However, the current system already has SQLite and CRUD (in `database.rs`) — it just doesn't use it. The event-sourced approach is actually *less* code than properly syncing mutable state across distributed binaries, because it removes the need for distributed locking and transaction coordination.

**The trait-per-port design adds indirection.** Every call becomes dynamic dispatch (`Arc<dyn Port>`) or requires heavy generics. In Rust, this adds `Box`ing and async-trait overhead. Mitigation: use `enum_dispatch` for known-small adapter sets in performance-critical paths, or monomorphize at the app layer.

**The outbox pattern adds latency.** Synchronous settlement (current mock) is <1ms. Outbox introduces at least one polling interval (say, 100ms). For high-frequency agent trading, this matters. Mitigation: the outbox can be bypassed for in-memory/test adapters; production adapters use it.

### Where Does Complexity Hide?

**Conflict resolution in event-sourced negotiations.** If buyer and seller both emit `Accepted` with different prices simultaneously (due to network partitions or malicious behavior), the state machine must define a winner. This requires vector clocks or logical timestamps, which the current design elides.

**Trust graph convergence.** Federated subjective trust sounds elegant but is computationally hard. PageRank on a dynamic graph with Sybil nodes is an open research problem. The "local graph" adapter is safe; the "federated" adapter needs bounded computation (e.g., max hop depth, trusted root set).

**MCP stdio transport limits deployment.** MCP over stdio means one process per client connection. For a high-throughput agent marketplace, HTTP or WebSocket transport is needed. The design must support multiple transport adapters simultaneously.

### Alternative Design A: CRUD-First with Projections

Instead of event sourcing, keep mutable state in PostgreSQL with row-level locking. Add read-only projections for analytics. This is simpler, familiar to most developers, and performs well for low-concurrency workloads.

**Why rejected:** DCAP's stated goal is "decentralized protocol." Decentralized systems cannot rely on a single PostgreSQL primary. Event sourcing is the natural representation for a protocol, because events are what cross the wire. Mutable state is a local optimization; events are the truth.

### Alternative Design B: Blockchain-Native

Put everything on Solana: negotiations as on-chain programs, settlement as native SOL/USDC transfers, trust as on-chain reputation tokens.

**Why rejected:** Blockchain latency (400ms-seconds) is too slow for multi-round negotiation. Cost per transaction ($0.001-$0.50) is too high for RFQ spam. Privacy is poor (all quotes public). The correct use of blockchain is for **settlement finality** and **trust attestation anchoring**, not for the negotiation lifecycle.

---

## Phase 5: Iterative Refinement

### Simplification Agenda

1. **Collapse the trait hierarchy for MVP.** Instead of 6 port traits with full generality, start with 3:
   - `EventStore` (append + read stream)
   - `Network` (send + receive envelope)
   - `Resolver` (identity → capabilities)

2. **Use `rust_decimal::Decimal` instead of `f64`.** Floating-point money is a bug. Every `f64` price in the current system is a precision trap.

3. **Replace the 15-variant `NegotiationError` with structured errors per port.**
   ```rust
   pub enum KernelError { InvalidTransition { ... }, SignatureInvalid, TTLExpired }
   pub enum StoreError { Io(io::Error), Serialize(serde_json::Error) }
   pub enum NetworkError { Timeout, Dns, Decode }
   ```

4. **Make `PaymentMethod` an open string with registry, not an enum.**
   ```rust
   pub struct PaymentMethod(String); // "stripe", "solana:usdc", "escrow:7day"
   pub struct PaymentMethodRegistry { ... }
   ```

5. **Defer federated trust to Phase 2.** Ship local-graph trust first. The port interface remains the same; only the adapter changes.

### Explicit Trade-Offs

| Trade-off | Choice | Rationale |
|-----------|--------|-----------|
| Performance vs Correctness | Correctness (event sourcing) | Commerce requires audit trails; speed can be optimized later |
| Flexibility vs Type Safety | Type safety (enums for core events) | Core protocol grammar must be rigid; extensibility lives in adapters |
| Decentralization vs Usability | Usability first | A centralized registry adapter ships now; DHT adapter replaces it later without app changes |
| Rust complexity vs Dev speed | Rust complexity | The domain (cryptography, async networking) justifies Rust; the protocol needs correctness |

---

## Phase 6: Convergence and Synthesis

### Final Architecture

The new "center of gravity" is the **Event-Sourced Negotiation Kernel.** Everything orbits this kernel.

```
┌─────────────────────────────────────────────────────────────┐
│                        DCAP KERNEL                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Identity   │  │   Commitment │  │ StateMachine │      │
│  │  (Ed25519)   │  │  (Signed+TTL)│  │(Pure fold)   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │    Event     │  │  Validation  │  │   Decimal    │      │
│  │   (Grammar)  │  │ (Invariants) │  │   (Money)    │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │ traits
┌─────────────────────────────────────────────────────────────┐
│                      PORT LAYER (traits)                    │
│   EventStore    Resolver    Network    Settlement  Trust    │
└─────────────────────────────────────────────────────────────┘
                              │ implementations
┌─────────────────────────────────────────────────────────────┐
│                    ADAPTER LAYER                            │
│  SQLite    HTTP    MCP stdio    Mock    Stripe    Solana    │
│  Memory    Axum    WebSocket    Null    (future)  (future)  │
└─────────────────────────────────────────────────────────────┘
                              │ wired by
┌─────────────────────────────────────────────────────────────┐
│                    APPLICATION LAYER                        │
│  buyer-agent    seller-agent    discovery    mcp-server     │
│  (thin binary)  (thin binary)   (thin bin)   (thin binary)  │
└─────────────────────────────────────────────────────────────┘
```

### Superiority Claims

**Compared to Original System:**
- **Original:** State scattered in HashMaps, lost on restart. **New:** All state derived from durable event stream.
- **Original:** Tight coupling — `BuyerAgent` directly constructs `SettlementService`. **New:** Agents know only the kernel and ports; adapters are injected.
- **Original:** Mock public keys, JWT shared secrets. **New:** Self-sovereign Ed25519 identity per message.
- **Original:** Broken custom TCP "MCP." **New:** Standards-compliant MCP adapter over stdio.
- **Original:** Scalar reputation (0-100). **New:** Multi-dimensional trust vector with category expertise.
- **Original:** Single error enum for everything. **New:** Structured errors per bounded context.
- **Original:** `f64` for money. **New:** `Decimal` everywhere.

**Compared to Initial Redesign:**
- **Initial:** 6 port traits, full generality. **Refined:** 3 core ports for MVP, with extension points.
- **Initial:** Federated trust graph. **Refined:** Local trust graph ships first; federated is a drop-in adapter.
- **Initial:** Full outbox + saga. **Refined:** Outbox for production adapters; synchronous in-memory path for tests.

### The New Center of Gravity

> **The event is the API.**

In the original system, the API was REST endpoints. In the new system, the API is the grammar of events. REST endpoints, MCP tools, WebSocket frames, and libp2p gossip are all just **transport encodings** of the same events. This means:
- Adding a new client (Discord bot, Slack integration, mobile app) requires only a new transport adapter, never kernel changes.
- Forking the protocol (for a private marketplace) means filtering events, not rewriting services.
- Auditing and compliance are trivial: replay the event stream.

---

## Phase 7: Forward Trajectory

### What Becomes Easier to Build

| Capability | How It Unlocks |
|------------|---------------|
| **Multi-party RFQs** | Event `RfqSubmitted` already supports multiple `recipient` identities. The kernel just needs to track `BTreeMap<Identity, Quote>` instead of `Option<Quote>`. |
| **Auctions** | New event variants (`BidPlaced`, `ReserveMet`, `AuctionExtended`) plug into the same state machine pattern. No new infrastructure. |
| **Subscription commerce** | `Commitment` already has TTL. Recurring commitments are just auto-generated RFQs with the same `subscription_id`. |
| **Human-in-the-loop** | A `HumanApprovalRequired` event pauses the state machine. An MCP prompt requests approval. The kernel resumes on `HumanApproved`. |
| **Compliance/auditing** | Regulators replay the event stream. No special audit API needed. |
| **LLM fine-tuning dataset** | Every negotiation is already an event stream — perfect training data for tactic prediction. |

### What New Capabilities Unlock

1. **Agent strategy marketplace.** Because `Strategy` is a trait, third parties can sell negotiation strategies as Rust dylibs or WASM modules. "The McKinsey strategy costs $0.01 per negotiation."

2. **Cross-protocol bridges.** A bridge adapter translates DCAP events to/from OpenID, ERC-4337, or Shopify webhooks. DCAP agents can negotiate with legacy e-commerce platforms.

3. **Zero-knowledge reputation.** A trust adapter using zk-SNARKs can prove "I have >100 successful transactions" without revealing counterparty identities or transaction details.

4. **Distributed discovery.** Replace the `registry.rs` adapter with a libp2p Kademlia adapter. Same port, new capability. No app changes.

### Next Scaling / Complexity Limits

| Horizon | Limit | Mitigation |
|---------|-------|------------|
| **~1K events/sec** | SQLite WAL mode saturates | Swap `sqlite` adapter for `postgresql` or `scylladb` event store |
| **~10K concurrent negotiations** | Single `AgentRuntime` actor mailbox | Shard by `NegotiationId` modulo N; each shard has its own runtime |
| **~100K agents** | Centralized registry lookup latency | Deploy DHT adapter; cache hot entries |
| **Global deployment** | Clock skew breaks TTL logic | Use logical clocks (Lamport timestamps) for event ordering; physical clocks only for TTL hints |
| **Regulatory fragmentation** | Different jurisdictions require different settlement rails | Jurisdiction-aware `Settlement` adapter routing |

---

## Appendix: Implementation Priority

### P0: Kernel Extraction (Week 1)
1. Create `kernel/` module with `Identity`, `Commitment`, `Event`, `StateMachine`
2. Replace `f64` with `Decimal` in kernel types
3. Write property-based tests for state machine (proptest)

### P1: Port/Adapter Refactor (Week 2)
1. Define `EventStore`, `Network`, `Resolver` traits
2. Move existing SQLite code into `adapters/persistence/sqlite.rs`
3. Move existing HTTP code into `adapters/transport/http.rs`
4. Wire in `lib.rs` with dependency injection

### P2: Agent Runtime (Week 3)
1. Implement `AgentRuntime` actor with mailbox
2. Port buyer/seller behavior into `Strategy` trait implementations
3. Ensure all state flows through `EventStore`

### P3: MCP Compliance (Week 4)
1. Replace custom TCP with `mcp_stdio` adapter
2. Map DCAP events to MCP tools/resources
3. Test with Claude Desktop / Inspector

### P4: Trust Graph (Week 5-6)
1. Implement `TrustEngine` port
2. Build `local_graph` adapter with category-specific reputation
3. Compute trust vectors from event stream

### P5: Production Hardening (Week 7-8)
1. Outbox pattern for settlement
2. Circuit breakers and retries on `Network`
3. Metrics and tracing (Prometheus + OpenTelemetry)

---

*End of Architectural Evolution Document*
