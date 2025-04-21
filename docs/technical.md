# Technical Specification

This document outlines the technical details, chosen technologies, and core component designs for the Chiti Identity project. It aims to provide a detailed blueprint for implementation.

## 1. Core Components & Modules

The system is designed around distinct Rust modules, interacting to provide the overall functionality. The core components map roughly to the following proposed modules:

1.  **`identity` Module (Conceptual: ID_MGR)**
    *   **Purpose**: Manages decentralized identifiers (DIDs), associated cryptographic keys (specifically the Beckn keypair), and links to the node's network identity.
    *   **Key Structs/Traits**:
        *   `struct Identity`: Holds the DID (e.g., `did:chiti:<SubspaceId25>`), the `iroh::net::NodeId`, the Beckn-specific keypair (e.g., `ed25519_dalek::Keypair`), and potentially metadata.
        *   `impl Identity`: Methods for generating new identities, signing Beckn messages (`fn sign(&self, message: &[u8]) -> Signature25`), verifying signatures (`fn verify(...)`).
        *   `trait IdentityStore`: An abstraction potentially implemented using the main `Store` trait, for loading/saving identity data (mapping `SubspaceId25` to `Identity` data).
    *   **Interaction**: Uses `willow_25` types for cryptographic elements. Relies on the `store` module for persistence. Provides keys for `service_adapter` signing needs.

2.  **`net` Module (Conceptual: NET_SVC - Transport & Sync Layer)**
    *   **Purpose**: Handles node-to-node communication, discovery, data transfer, and synchronization using Iroh and `iroh-willow`.
    *   **Key Structs/Functionality**:
        *   `struct NetworkManager`: Manages the `iroh::net::Endpoint`.
        *   Initialization: Setting up the `iroh::net::Endpoint` with appropriate key material (linking to `identity::Identity::node_id`).
        *   Connection Management: Using `endpoint.connect(node_id, addrs)` or `endpoint.connect_ticket(...)`. Handling connection events.
        *   Synchronization: Initiating and managing `iroh_willow::SyncHandle` sessions between connected peers for specific `NamespaceId25`. Relies on the `store` module to provide the underlying `Store` implementation for sync.
    *   **Interaction**: Provides connectivity services for `service_adapter` (e.g., for registry lookups or direct BAP/BPP communication). Uses the `store` module as the data source/sink for synchronization.

3.  **`store` Module (Conceptual: DATA_STORE)**
    *   **Purpose**: Provides a concrete implementation of the `willow_data_model::Store` trait for persistent data storage.
    *   **Key Structs/Functionality**:
        *   `struct WillowSledStore`: A wrapper around `willow_store_simple_sled::StoreSimpleSled`.
        *   `impl willow_data_model::Store for WillowSledStore`: Implements all methods required by the `Store` trait (`get`, `put`, `subscribe_area`, etc.), delegating calls to the underlying `sled`-based store.
        *   Configuration: Managing the `sled::Db` instance.
    *   **Interaction**: Used by `net` for synchronization, by `identity` for persisting identity data, by `access` for capability checks (if needed implicitly by the store), and by `service_adapter` (via `logic`) for reading/writing transaction state.

4.  **`access` Module (Conceptual: ACCESS_CTRL)**
    *   **Purpose**: Manages and enforces access control based on Meadowcap capabilities.
    *   **Key Structs/Functionality**:
        *   `struct AccessManager`: Provides helper functions for capability management.
        *   Capability Creation: Functions to create `meadowcap::OwnedCapability` or `CommunalCapability` based on `willow_25` parameters (e.g., `fn create_owned_write_capability(...) -> McCapability<...>` ).
        *   Authorisation Token Generation: Creating `meadowcap::McAuthorisationToken` using the capabilities and the identity's signing key (`identity::Identity::sign`).
        *   Verification Logic: Although verification (`is_authorised_write`) happens within the `Store` or `AuthorisedEntry` structure itself, this module might provide validation helpers if needed before attempting writes.
    *   **Interaction**: Used by components needing to generate authorisation tokens (e.g., `service_adapter` before writing data via the `store`). Relies on `identity` for signing keys and uses types from `meadowcap` and `willow_25`.

5.  **`service_adapter` Module (Conceptual: BAP_IF, BPP_IF, REG_CLIENT, MSG_VALIDATOR)**
    *   **Purpose**: Implements the Beckn protocol logic, handling specific service interactions (generically), validation, and registry communication. Designed to be generic over different Beckn service types.
    *   **Key Structs/Traits/Modules**:
        *   `trait ServiceHandler`: Defines the interface for a specific Beckn service (e.g., retail, mobility). Implementors would handle service-specific state transitions, data structures, and validation logic.
            *   `type State: BecknTransactionState`: Associated type for the service's transaction state.
            *   `fn handle_action(&self, action: BecknAction, context: ActionContext) -> Result<StateChange, ServiceError>;`
            *   `fn validate_payload(&self, action: BecknAction, payload: &serde_json::Value) -> Result<(), ValidationError>;`
        *   `struct BecknAdapter<S: ServiceHandler>`: The main adapter, generic over a `ServiceHandler`. Manages incoming requests, delegates to the specific `ServiceHandler`, interacts with `net`, `store` (via `logic`), and `access`.
        *   `mod registry`: (Conceptual: REG_CLIENT)
            *   `async fn register_identity(net: &NetworkManager, id: &Identity, registry_node: NodeId) -> Result<...>;`
            *   `async fn lookup_role(net: &NetworkManager, role: &str, domain: &str, registry_node: NodeId) -> Result<Vec<NodeId>, ...>;`
        *   `mod validation`: (Conceptual: MSG_VALIDATOR) Uses standard Beckn schemas (potentially via generated Rust types from OpenAPI spec) and delegates service-specific validation to `ServiceHandler::validate_payload`.
        *   `mod logic`: (Conceptual: TXN_LOGIC)
            *   `trait BecknTransactionState`: Core state shared across Beckn transactions (e.g., transaction ID, status).
            *   `async fn load_state<T: BecknTransactionState + ServiceHandler::State>(store: &impl Store, txn_id: &str) -> Result<T, ...>;`
            *   `async fn save_state<T: BecknTransactionState + ServiceHandler::State>(store: &impl Store, state: &T, auth_token: &McAuthorisationToken<...>) -> Result<...>;` (Handles writing state changes to the Willow store using appropriate paths).
    *   **Interaction**: Entry point for external Beckn interactions. Uses `net` for communication, `store` for persistence, `access` for authorisation, `identity` for signing. Defines the core generic framework for Beckn services.

## 2. Tech Stack

*   **Primary Language**: Rust (Stable toolchain)
*   **Core Crates**:
    *   `tokio`: Asynchronous runtime.
    *   `iroh` (v0.15.0+): Networking (`iroh::net`), cryptography (`iroh::crypto`).
    *   `willow-data-model` (v0.2.0+): Core Willow traits (`Store`, `NamespaceId`, etc.) and structures (`Path`, `Entry`).
    *   `meadowcap` (v0.2.0+): Meadowcap capability implementation (`McCapability`, `McAuthorisationToken`).
    *   `willow_25` (v0.1.0+): Concrete Willow/Meadowcap parameters (Ed25519, Blake3 based types like `SubspaceId25`, `Signature25`, `PayloadDigest25`).
    *   `iroh-willow` (latest compatible): Willow General Purpose Sync (WGPS) protocol implementation over Iroh.
    *   `willow-store-simple-sled` (v0.1.0+): Initial `Store` trait implementation using `sled`.
    *   `sled` (v0.34+): Embedded key-value store backend.
    *   `serde`, `serde_json`: Data serialization/deserialization.
    *   `sqlx` or `diesel`: For interacting with the PostgreSQL `AuditDB`.
    *   `tracing`: Logging and diagnostics framework.
    *   `thiserror`: Error handling boilerplate.
*   **External Dependencies**:
    *   PostgreSQL: For relational audit logs (`AuditDB`).

## 3. Data Structures & Persistence (Willow/Sled & PostgreSQL)

### Willow Data Mapping (`store` module via `willow-store-simple-sled`)

*   **`NamespaceId`**: Likely a single, fixed `NamespaceId25` for the entire application instance.
*   **`SubspaceId`**: Maps to the public key of a user's identity (`willow_25::SubspaceId25`, derived from `identity::Identity`'s key). Each user controls their own subspace.
*   **`Path`**: Used hierarchically within a subspace to organize data:
    *   `/identity/profile`: Public profile information.
    *   `/identity/beckn_key`: Public part of the Beckn key.
    *   `/transactions/<domain>/<txn_id>/<action>`: Records Beckn transaction states (e.g., `/transactions/retail/abc-123/on_select`). The payload would be the JSON state.
    *   `/capabilities/granted/<capability_hash>`: Record of capabilities granted.
*   **`Entry`**: Represents a specific data point at a `Path` with a `Timestamp` and `PayloadDigest`.
*   **`PayloadDigest`**: `willow_25::PayloadDigest25` (Blake3 hash) of the actual payload data (e.g., serialized JSON state, profile info). The store manages payload storage.
*   **`AuthorisationToken`**: `meadowcap::McAuthorisationToken<..., willow_25::Signature25>` attached to `Entry` structs to prove write permission based on `meadowcap` capabilities.
*   **Storage Backend**: `willow-store-simple-sled` uses `sled`. *Caveat: Loads entire payloads into memory, potentially impacting performance for very large payloads.*

### Audit Logging (`AuditDB` via `sqlx`/`diesel`)

*   **Purpose**: Provides a tamper-evident, relational log of significant events for auditing and debugging, separate from the eventually consistent Willow store.
*   **Database**: PostgreSQL.
*   **Interaction**: Using `sqlx` (async) or `diesel`.
*   **Example Table (`audit_log`)**:
    *   `id`: BIGSERIAL PRIMARY KEY
    *   `timestamp`: TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
    *   `event_type`: VARCHAR(255) NOT NULL (e.g., 'BECKN_ACTION', 'IDENTITY_CREATED', 'SYNC_STARTED')
    *   `actor_node_id`: VARCHAR(255) (Iroh NodeId)
    *   `actor_did`: VARCHAR(255) (Chiti DID)
    *   `target`: VARCHAR(255) (e.g., Transaction ID, Peer NodeId)
    *   `details`: JSONB (Context-specific information)

## 4. Networking Layer (Iroh via `net` module)

*   **Core**: `iroh::net::Endpoint` manages connections and network state.
*   **Identity**: Each instance runs an Endpoint identified by a `iroh::net::NodeId` (derived from an Ed25519 keypair, linked to `identity::Identity`).
*   **Discovery & Connection**:
    *   Direct connection via `endpoint.connect(node_id, addrs)` if peer details are known (e.g., from registry).
    *   Ticket-based connection (`iroh::net::Ticket`) for easy out-of-band sharing.
    *   Potential use of future Iroh discovery mechanisms if needed.
*   **Transport**: Secure, multiplexed connections via QUIC (provided by Iroh). Includes NAT traversal capabilities.
*   **Synchronization**: `iroh-willow` leverages Iroh connections to run the Willow General Purpose Sync (WGPS) protocol, synchronizing `Entry` data within specified `NamespaceId`s based on peer interests and capabilities, using the `store::WillowSledStore` as the data source/sink.

## 5. Security Implementation

*   **Node Authentication**: Handled natively by Iroh. Connections between nodes are authenticated using the Ed25519 keys underlying their `NodeId`.
*   **Data Access Control (Willow)**: Enforced by `meadowcap` capabilities. Writes to the `store` require a valid `McAuthorisationToken` signed by an identity holding an appropriate `McCapability` for the target `Path` and `SubspaceId`. Verification occurs implicitly when adding `AuthorisedEntry` data. Managed conceptually by the `access` module.
*   **Beckn Message Security**:
    *   Signatures: Beckn actions requiring signatures (as per spec) will be signed by the `identity::Identity`'s Beckn keypair.
    *   Verification: Incoming signed messages will be verified against the claimed sender's public key (potentially retrieved via the `store` or registry lookup).
*   **Transport Security**: Provided by Iroh using QUIC/TLS, securing data in transit between nodes.
*   **Audit Trail**: The `AuditDB` provides an immutable log for security monitoring.

## 6. Development Workflow & Testing

*   **Workflow**: Standard Rust development:
    *   `cargo build`
    *   `cargo run`
    *   `cargo test`
    *   `cargo clippy --all-targets --all-features`
    *   `cargo fmt`
    *   `cargo doc --open`
*   **Testing Strategy**:
    *   **Unit Tests (`#[cfg(test)]`)**: Test individual functions, struct methods, and logic within modules. Use mocking libraries (e.g., `mockall`) to isolate dependencies (like `Store` interactions, network calls).
    *   **Integration Tests (`tests/` directory)**: Test interactions between modules (e.g., `service_adapter` -> `logic` -> `store`). May involve setting up temporary `sled::Db` instances.
    *   **Network/Sync Tests**: Utilize `iroh::test_utils` or similar frameworks if applicable. Alternatively, write tests that spawn multiple `NetworkManager` instances in separate Tokio tasks on localhost, connect them, and verify `iroh-willow` sync behavior using test data in their respective `Store` instances.
    *   **Willow Model Testing**: Consider property-based testing (`proptest`) for validating invariants in custom logic interacting with Willow paths, entries, or capabilities.
    *   **Beckn Conformance**: Validate against official Beckn schemas/APIs. Use or develop test suites simulating BAP/BPP interactions for specific service types implemented via the `ServiceHandler` trait. End-to-end tests simulating full transaction flows are crucial.

## 7. Deployment & Scaling Considerations

*   **Deployment**:
    *   Build optimized Rust static binaries (`cargo build --release`).
    *   Deploy binaries directly or containerize using Docker.
    *   **Conceptual Dockerfile:**
        ```dockerfile
        # Stage 1: Build
        FROM rust:stable AS builder
        WORKDIR /usr/src/chiti
        COPY . .
        # Install build dependencies if any (e.g., protobuf compiler)
        # RUN apt-get update && apt-get install -y protobuf-compiler
        RUN cargo build --release --locked

        # Stage 2: Runtime
        FROM debian:bullseye-slim
        # Install runtime dependencies if any (e.g., ca-certificates)
        RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
        COPY --from=builder /usr/src/chiti/target/release/chiti-identity /usr/local/bin/chiti-identity
        # Copy configuration files if needed
        # COPY config.toml /etc/chiti/config.toml

        # Set up non-root user?

        EXPOSE 4433 # Default Iroh QUIC port (or configure differently)
        CMD ["chiti-identity"]
        ```
*   **Scaling & Performance**:
    *   **State**: The system is stateful (Iroh connections, Willow store). Scaling typically involves running more independent nodes rather than stateless replicas behind a load balancer.
    *   **Bottlenecks**:
        *   `willow-store-simple-sled`: Performance under high write load or with large payloads needs monitoring. Consider alternative `Store` backends if needed.
        *   Sync Complexity: Complex sync queries or large numbers of peers could strain resources. Optimize `iroh-willow` usage and potentially filter sync interests.
        *   CPU: Cryptographic operations (signing, hashing, verification) can be CPU-intensive.
    *   **Mitigation**: Vertical scaling (more CPU/RAM per node) is the primary approach. Application-level sharding (distributing DIDs/Subspaces across multiple node instances) could be explored if single-node limits are hit.
    *   **Monitoring**: Use standard OS-level monitoring (CPU, RAM, disk I/O, network). Integrate with Prometheus via exporters (e.g., `prometheus_exporter`) for application-level metrics (sync progress, queue lengths, API timings). Use `tracing` spans for performance analysis.

## 8. Future Considerations

*   **Alternative `Store` Implementations**: Investigate or develop `Store` backends using RocksDB, PostgreSQL (potentially via `sqlx`), or other persistent stores if `sled` limitations become problematic.
*   **Cryptography**: Allow configuration of different cryptographic suites via Willow parameters beyond `willow_25` if required by specific consortia or standards.
*   **Willow E2E Encryption**: Explore implementing payload encryption/decryption strategies compatible with Willow.
*   **Advanced Capabilities**: Utilize more complex `meadowcap` features if needed (e.g., capability revocation, delegation chains).
*   **Wider Beckn Role/Service Support**: Implement more `ServiceHandler` traits for various Beckn domains. Add support for gateway roles if necessary.
*   **Iroh Evolution**: Adapt to new features in Iroh (e.g., enhanced discovery, pub/sub).