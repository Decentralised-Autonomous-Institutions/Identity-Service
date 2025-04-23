# Technical Specification

This document outlines the technical details, chosen technologies, and core component designs for the Chiti Identity project. It aims to provide a detailed blueprint for implementation.

## 1. System Architecture Overview

The Chiti Identity system follows a layered architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                Application Layer                            │
│     (BAP/BPP Applications, Registry Clients, User UIs)      │
└───────────────────────────────┬─────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────┐
│               Service Adaptation Layer                      │
│  ┌─────────────────┐           ┌────────────────────────┐   │
│  │  BAP_Interface  │           │  BPP_Interface         │   │
│  └─────────────────┘           └────────────────────────┘   │
└───────────────────────────────┬─────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────┐
│                Core Services Layer                          │
│  ┌────────────┐ ┌────────┐ ┌─────────┐ ┌────────┐ ┌───────┐ │
│  │ identity   │ │ net    │ │ store   │ │ access │ │ AUDIT │ │
│  └────────────┘ └────────┘ └─────────┘ └────────┘ └───────┘ │
└───────────────────────────────┬─────────────────────────────┘
                                │
┌───────────────────────────────▼─────────────────────────────┐
│             Data Persistence & External Systems             │
│         (Willow/Sled Store, PostgreSQL, Registry)           │
└─────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Decentralization**: Using DIDs and distributed data storage for identity sovereignty
2. **Modularity**: Each component has a single responsibility and clear interfaces
3. **Extensibility**: New Beckn domains can be added without modifying core logic
4. **Security**: Capability-based access control with cryptographic verification
5. **Auditability**: Complete logging of all system events
6. **Resilience**: Robust error handling and recovery strategies
7. **Interoperability**: Compliance with Beckn protocol specifications

## 2. Core Components & Modules

The system is designed around distinct Rust modules, interacting to provide the overall functionality. The core components map roughly to the following proposed modules:

1.  **`identity` Module (Conceptual: ID_MGR)**
    *   **Purpose**: Manages decentralized identifiers (DIDs), associated cryptographic keys (specifically the Beckn keypair), and links to the node's network identity.
    *   **Key Structs/Traits**:
        *   `struct Identity`: Holds the DID (e.g., `did:<NamespaceId25>:<SubspaceId25>`), the `iroh::net::NodeId`, the Beckn-specific keypair (e.g., `ed25519_dalek::Keypair`), and potentially metadata.
        *   `impl Identity`: Methods for generating new identities, signing Beckn messages (`fn sign(&self, message: &[u8]) -> Signature25`), verifying signatures (`fn verify(...)`).
        *   `trait IdentityStore`: An abstraction potentially implemented using the main `Store` trait, for loading/saving identity data (mapping `SubspaceId25` to `Identity` data).
    *   **Interaction**: Uses `willow_25` types for cryptographic elements. Relies on the `store` module for persistence. Provides keys for `service_adapter` signing needs.
    *   **Detailed Data Structures**:
    ```rust
    pub struct Identity {
        /// The decentralized identifier in the format did:<NamespaceId25>:<SubspaceId25>
        did: String,
        /// The cryptographic keypair for Beckn protocol operations
        beckn_keypair: ed25519_dalek::Keypair,
        /// The network identity used by Iroh for p2p communication
        node_id: Option<iroh::net::NodeId>,
        /// Additional information about this identity
        metadata: IdentityMetadata,
    }

    pub struct IdentityMetadata {
        /// Human-readable name for this identity
        name: String,
        /// Role in the Beckn network (BAP, BPP, BG, Registry)
        beckn_role: BecknRole,
        /// Domains this identity participates in (e.g., mobility, retail)
        domains: Vec<String>,
        /// When this identity was created
        created_at: chrono::DateTime<chrono::Utc>,
        /// When this identity was last updated
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    pub enum BecknRole {
        BAP,
        BPP,
        BG,
        Registry,
    }
    ```
    *   **Core Functions**:
    ```rust
    impl Identity {
        /// Create a new identity with a fresh keypair
        pub fn new(name: String, role: BecknRole, domains: Vec<String>) -> Self { ... }
        
        /// Load an identity from an existing keypair
        pub fn from_keypair(keypair_bytes: &[u8], name: String, role: BecknRole, 
                            domains: Vec<String>) -> Result<Self, IdentityError> { ... }
        
        /// Sign data using this identity's keypair
        pub fn sign(&self, data: &[u8]) -> willow_25::Signature25 { ... }
        
        /// Verify a signature against this identity's public key
        pub fn verify(&self, data: &[u8], signature: &willow_25::Signature25) -> bool { ... }
        
        /// Create a Beckn authorization header
        pub fn create_auth_header(&self, request_body: &str) -> String { ... }
        
        /// Get the DID string representation
        pub fn as_did_string(&self) -> &str { ... }
        
        /// Export the keypair for backup (should be encrypted before storage)
        pub fn export_keypair(&self) -> Vec<u8> { ... }
        
        /// Get the SubspaceId25 derived from this identity's public key
        pub fn subspace_id(&self) -> willow_25::SubspaceId25 { ... }
    }
    ```
    *   **Storage Interface**:
    ```rust
    pub trait IdentityStore {
        /// Save an identity to the store
        async fn save_identity(&self, identity: &Identity) -> Result<(), IdentityStoreError>;
        
        /// Load an identity by DID
        async fn load_identity(&self, did: &str) -> Result<Identity, IdentityStoreError>;
        
        /// List all identities
        async fn list_identities(&self) -> Result<Vec<IdentitySummary>, IdentityStoreError>;
        
        /// Delete an identity
        async fn delete_identity(&self, did: &str) -> Result<(), IdentityStoreError>;
    }
    
    pub struct IdentitySummary {
        did: String,
        name: String,
        beckn_role: BecknRole,
        domains: Vec<String>,
        created_at: chrono::DateTime<chrono::Utc>,
    }
    ```

2.  **`net` Module (Conceptual: NET_SVC - Transport & Sync Layer)**
    *   **Purpose**: Handles node-to-node communication, discovery, data transfer, and synchronization using Iroh and `iroh-willow`.
    *   **Key Structs/Functionality**:
        *   `struct NetworkManager`: Manages the `iroh::net::Endpoint`.
        *   Initialization: Setting up the `iroh::net::Endpoint` with appropriate key material (linking to `identity::Identity::node_id`).
        *   Connection Management: Using `endpoint.connect(node_id, addrs)` or `endpoint.connect_ticket(...)`. Handling connection events.
        *   Synchronization: Initiating and managing `iroh_willow::SyncHandle` sessions between connected peers for specific `NamespaceId25`. Relies on the `store` module to provide the underlying `Store` implementation for sync.
    *   **Interaction**: Provides connectivity services for `service_adapter` (e.g., for registry lookups or direct BAP/BPP communication). Uses the `store` module as the data source/sink for synchronization.
    *   **Detailed Data Structures**:
    ```rust
    pub struct NetworkManager {
        /// The Iroh network endpoint for P2P communication
        endpoint: iroh::net::Endpoint,
        /// Active connections to peers
        active_connections: std::collections::HashMap<iroh::net::NodeId, ConnectionInfo>,
        /// Sync handles for Willow namespaces
        sync_handles: std::collections::HashMap<willow_25::NamespaceId25, iroh_willow::SyncHandle>,
        /// Network configuration
        config: NetworkConfig,
        /// Channel for connection events
        connection_events: tokio::sync::broadcast::Sender<ConnectionEvent>,
    }

    pub struct ConnectionInfo {
        node_id: iroh::net::NodeId,
        addresses: Vec<std::net::SocketAddr>,
        established_at: chrono::DateTime<chrono::Utc>,
        connection_type: ConnectionType,
        state: ConnectionState,
    }

    pub enum ConnectionType {
        Direct,
        TicketBased,
        DiscoveryBased,
    }

    pub enum ConnectionState {
        Connecting,
        Connected,
        Disconnected,
        Failed { reason: String },
    }

    pub struct NetworkConfig {
        listen_addr: std::net::SocketAddr,
        known_peers: Vec<KnownPeer>,
        sync_interval_seconds: u64,
        connection_timeout_seconds: u64,
        max_concurrent_connections: usize,
    }

    pub struct KnownPeer {
        node_id: iroh::net::NodeId,
        addresses: Vec<std::net::SocketAddr>,
        auto_connect: bool,
    }

    pub enum ConnectionEvent {
        Connected {
            node_id: iroh::net::NodeId,
            addresses: Vec<std::net::SocketAddr>,
        },
        Disconnected {
            node_id: iroh::net::NodeId,
            reason: Option<String>,
        },
        Failed {
            node_id: iroh::net::NodeId,
            error: String,
        },
    }
    ```
    *   **Core Functions**:
    ```rust
    impl NetworkManager {
        /// Create a new network manager with the given configuration
        pub async fn new(identity: &Identity, config: NetworkConfig) -> Result<Self, NetworkError> { ... }
        
        /// Connect to a peer using their node ID and addresses
        pub async fn connect_direct(
            &mut self, 
            node_id: iroh::net::NodeId, 
            addresses: Vec<std::net::SocketAddr>
        ) -> Result<ConnectionInfo, NetworkError> { ... }
        
        /// Connect to a peer using a connection ticket
        pub async fn connect_ticket(
            &mut self, 
            ticket: &str
        ) -> Result<ConnectionInfo, NetworkError> { ... }
        
        /// Generate a connection ticket for other peers to connect to this node
        pub fn create_connection_ticket(&self) -> Result<String, NetworkError> { ... }
        
        /// Start synchronizing a namespace with a peer
        pub async fn sync_namespace(
            &mut self, 
            namespace: willow_25::NamespaceId25, 
            store: &dyn willow_data_model::Store,
            node_id: iroh::net::NodeId
        ) -> Result<iroh_willow::SyncHandle, NetworkError> { ... }
        
        /// List all active connections
        pub fn list_connections(&self) -> Vec<ConnectionInfo> { ... }
        
        /// Send data directly to a peer (not using Willow sync)
        pub async fn send_data(
            &self, 
            node_id: iroh::net::NodeId, 
            data: Vec<u8>
        ) -> Result<(), NetworkError> { ... }
        
        /// Receive data from a specified peer
        pub async fn receive_data(
            &self, 
            node_id: iroh::net::NodeId
        ) -> Result<Option<Vec<u8>>, NetworkError> { ... }
        
        /// Subscribe to connection events
        pub fn subscribe_connection_events(
            &self
        ) -> tokio::sync::broadcast::Receiver<ConnectionEvent> { ... }
        
        /// Close a specific connection
        pub async fn disconnect(
            &mut self, 
            node_id: iroh::net::NodeId
        ) -> Result<(), NetworkError> { ... }
    }
    ```

3.  **`store` Module (Conceptual: DATA_STORE)**
    *   **Purpose**: Provides a concrete implementation of the `willow_data_model::Store` trait for persistent data storage.
    *   **Key Structs/Functionality**:
        *   `struct WillowSledStore`: A wrapper around `willow_store_simple_sled::StoreSimpleSled`.
        *   `impl willow_data_model::Store for WillowSledStore`: Implements all methods required by the `Store` trait (`get`, `put`, `subscribe_area`, etc.), delegating calls to the underlying `sled`-based store.
        *   Configuration: Managing the `sled::Db` instance.
    *   **Interaction**: Used by `net` for synchronization, by `identity` for persisting identity data, by `access` for capability checks (if needed implicitly by the store), and by `service_adapter` (via `logic`) for reading/writing transaction state.
    *   **Detailed Data Structures**:
    ```rust
    pub struct WillowSledStore {
        /// The underlying Willow store implementation
        inner: willow_store_simple_sled::StoreSimpleSled,
        /// The namespace identifier for this store
        namespace: willow_25::NamespaceId25,
        /// Configuration for this store
        config: StoreConfig,
        /// Event subscribers
        subscribers: std::sync::Arc<tokio::sync::RwLock<Subscribers>>,
    }

    pub struct StoreConfig {
        /// Path to the database directory
        db_path: std::path::PathBuf,
        /// Maximum size of cache (in bytes)
        cache_size_bytes: usize,
        /// Whether to flush writes synchronously
        flush_every_write: bool,
        /// Automatic compaction settings
        compaction: CompactionConfig,
        /// Backup configuration
        backup: BackupConfig,
    }

    pub struct CompactionConfig {
        /// Whether to enable automatic compaction
        enabled: bool,
        /// Minimum bytes to compact
        min_bytes: u64,
        /// How frequently to check for compaction needs (in seconds)
        check_interval_seconds: u64,
    }

    pub struct BackupConfig {
        /// Whether to enable automatic backups
        enabled: bool,
        /// Directory to store backups
        backup_dir: std::path::PathBuf,
        /// Interval between backups (in hours)
        interval_hours: u64,
        /// Number of backups to retain
        retention_count: usize,
    }

    /// Represents subscribers to store events
    type Subscribers = std::collections::HashMap<
        willow_data_model::AreaFilter, 
        Vec<tokio::sync::mpsc::UnboundedSender<willow_data_model::StoreEvent>>
    >;

    pub struct StoreStats {
        /// Total number of entries
        entry_count: usize,
        /// Total size of payloads in bytes
        payload_size_bytes: u64,
        /// Number of subspaces
        subspace_count: usize,
        /// Database size on disk
        database_size_bytes: u64,
        /// Memory usage
        memory_usage_bytes: u64,
    }
    ```
    *   **Core Functions**:
    ```rust
    impl WillowSledStore {
        /// Create a new WillowSledStore with the given configuration
        pub fn new(
            config: StoreConfig, 
            namespace: Option<willow_25::NamespaceId25>
        ) -> Result<Self, StoreError> { ... }
        
        /// Get or create a subspace for an identity
        pub fn get_or_create_subspace(
            &self, 
            identity: &Identity
        ) -> Result<willow_25::SubspaceId25, StoreError> { ... }
        
        /// Helper to construct a Willow path from components
        pub fn build_path(
            components: &[&str]
        ) -> Result<willow_data_model::Path, StoreError> { ... }
        
        /// Get statistics about the store
        pub fn get_stats(&self) -> Result<StoreStats, StoreError> { ... }
        
        /// Perform a backup of the database
        pub async fn backup(&self) -> Result<std::path::PathBuf, StoreError> { ... }
        
        /// Restore from a backup
        pub async fn restore_from_backup(
            &self, 
            backup_path: &std::path::Path
        ) -> Result<(), StoreError> { ... }
        
        /// Compact the database to reclaim space
        pub async fn compact(&self) -> Result<(), StoreError> { ... }
        
        /// List all subspaces in the store
        pub fn list_subspaces(&self) -> Result<Vec<willow_25::SubspaceId25>, StoreError> { ... }
        
        /// List entries in a specific area (for debugging/exploration)
        pub async fn list_entries(
            &self,
            subspace: &willow_25::SubspaceId25,
            path_prefix: &str,
            limit: usize
        ) -> Result<Vec<(willow_data_model::Path, willow_data_model::Entry)>, StoreError> { ... }
    }

    impl willow_data_model::Store for WillowSledStore {
        async fn get(
            &self,
            subspace: &willow_25::SubspaceId25,
            path: &willow_data_model::Path
        ) -> Result<Option<willow_data_model::Entry>, willow_data_model::StoreError> { ... }
        
        async fn get_with_payload(
            &self,
            subspace: &willow_25::SubspaceId25,
            path: &willow_data_model::Path
        ) -> Result<Option<(willow_data_model::Entry, Vec<u8>)>, willow_data_model::StoreError> { ... }
        
        async fn put(
            &self,
            subspace: &willow_25::SubspaceId25,
            path: &willow_data_model::Path,
            payload: Vec<u8>,
            auth_token: Option<meadowcap::McAuthorisationToken<...>>
        ) -> Result<willow_data_model::Entry, willow_data_model::StoreError> { ... }
        
        async fn delete(
            &self,
            subspace: &willow_25::SubspaceId25,
            path: &willow_data_model::Path,
            auth_token: Option<meadowcap::McAuthorisationToken<...>>
        ) -> Result<(), willow_data_model::StoreError> { ... }
        
        async fn subscribe_area(
            &self,
            area_filter: willow_data_model::AreaFilter
        ) -> Result<
            tokio::sync::mpsc::UnboundedReceiver<willow_data_model::StoreEvent>,
            willow_data_model::StoreError
        > { ... }
        
        // Other Store trait methods
    }
    ```

4.  **`access` Module (Conceptual: ACCESS_CTRL)**
    *   **Purpose**: Manages and enforces access control based on Meadowcap capabilities.
    *   **Key Structs/Functionality**:
        *   `struct AccessManager`: Provides helper functions for capability management.
        *   Capability Creation: Functions to create `meadowcap::OwnedCapability` or `CommunalCapability` based on `willow_25` parameters (e.g., `fn create_owned_write_capability(...) -> McCapability<...>` ).
        *   Authorisation Token Generation: Creating `meadowcap::McAuthorisationToken` using the capabilities and the identity's signing key (`identity::Identity::sign`).
        *   Verification Logic: Although verification (`is_authorised_write`) happens within the `Store` or `AuthorisedEntry` structure itself, this module might provide validation helpers if needed before attempting writes.
    *   **Interaction**: Used by components needing to generate authorisation tokens (e.g., `service_adapter` before writing data via the `store`). Relies on `identity` for signing keys and uses types from `meadowcap` and `willow_25`.
    *   **Detailed Data Structures**:
    ```rust
    pub struct AccessManager {
        /// The identity used for signing capabilities and tokens
        identity: std::sync::Arc<Identity>,
        /// The Willow store used for querying capability state
        store: std::sync::Arc<WillowSledStore>,
    }

    pub enum Operation {
        Read,
        Write,
        Delete,
        Grant,
    }

    pub struct PathPattern {
        /// The pattern string (e.g., "/transactions/retail/*")
        pattern: String,
        /// Whether this is an exact path or a pattern with wildcards
        is_pattern: bool,
    }

    pub struct CapabilityGrant {
        /// The DID of the identity receiving the capability
        grantee_did: String,
        /// The subspace this capability applies to
        subspace: willow_25::SubspaceId25,
        /// The path pattern this capability applies to
        path_pattern: PathPattern,
        /// The operations allowed by this capability
        operations: Vec<Operation>,
        /// When this capability expires (if applicable)
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        /// Whether this capability can be delegated further
        delegatable: bool,
    }

    /// Wrapper type for a meadowcap capability with metadata
    pub struct StoredCapability {
        /// The actual capability
        capability: meadowcap::OwnedCapability<willow_25::PayloadDigest25, willow_25::Signature25>,
        /// The metadata about this capability
        metadata: CapabilityMetadata,
    }

    pub struct CapabilityMetadata {
        /// Unique identifier for this capability
        id: String,
        /// Human-readable description
        description: Option<String>,
        /// When this capability was created
        created_at: chrono::DateTime<chrono::Utc>,
        /// The DID of the identity that granted this capability
        grantor_did: String,
        /// The DID of the identity that received this capability
        grantee_did: String,
    }
    ```
    *   **Core Functions**:
    ```rust
    impl AccessManager {
        /// Create a new AccessManager
        pub fn new(
            identity: std::sync::Arc<Identity>, 
            store: std::sync::Arc<WillowSledStore>
        ) -> Self { ... }
        
        /// Grant a capability to another identity
        pub async fn grant_capability(
            &self, 
            grant: CapabilityGrant
        ) -> Result<StoredCapability, AccessError> { ... }
        
        /// Create an authorization token for a write operation
        pub async fn create_auth_token(
            &self,
            capability: &meadowcap::OwnedCapability<willow_25::PayloadDigest25, willow_25::Signature25>,
            path: &willow_data_model::Path,
            operation: Operation
        ) -> Result<meadowcap::McAuthorisationToken<willow_25::PayloadDigest25, willow_25::Signature25>, AccessError> { ... }
        
        /// Verify if a capability is valid for a path and operation
        pub async fn verify_capability(
            &self,
            capability: &meadowcap::OwnedCapability<willow_25::PayloadDigest25, willow_25::Signature25>,
            path: &willow_data_model::Path,
            operation: Operation
        ) -> Result<bool, AccessError> { ... }
        
        /// Revoke a previously granted capability
        pub async fn revoke_capability(
            &self,
            capability_id: &str
        ) -> Result<(), AccessError> { ... }
        
        /// List all capabilities granted by this identity
        pub async fn list_granted_capabilities(
            &self
        ) -> Result<Vec<StoredCapability>, AccessError> { ... }
        
        /// List all capabilities received by this identity
        pub async fn list_received_capabilities(
            &self
        ) -> Result<Vec<StoredCapability>, AccessError> { ... }
        
        /// Create a communal capability (shared across many identities)
        pub async fn create_communal_capability(
            &self,
            path_pattern: PathPattern,
            operations: Vec<Operation>
        ) -> Result<meadowcap::CommunalCapability<willow_25::PayloadDigest25>, AccessError> { ... }
    }
    ```

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
    *   **Detailed Data Structures**:
    ```rust
    /// The main Beckn action types
    pub enum BecknAction {
        Search,
        Select,
        Init,
        Confirm,
        Status,
        Track,
        Cancel,
        Update,
        Rating,
        Support,
        OnSearch,
        OnSelect,
        OnInit,
        OnConfirm,
        OnStatus,
        OnTrack,
        OnCancel,
        OnUpdate,
        OnRating,
        OnSupport,
    }

    /// Context for executing a Beckn action
    pub struct ActionContext {
        /// The transaction ID if available
        transaction_id: Option<String>,
        /// The identity making the request
        requester: std::sync::Arc<Identity>,
        /// The action's message body
        message: serde_json::Value,
        /// The action's context object
        context: BecknContext,
        /// Additional metadata for processing
        metadata: std::collections::HashMap<String, String>,
    }

    /// Beckn protocol context object
    pub struct BecknContext {
        /// Protocol version
        version: String,
        /// Transaction ID
        transaction_id: Option<String>,
        /// Message ID
        message_id: String,
        /// Action being performed
        action: String,
        /// Domain of the transaction
        domain: String,
        /// Timestamp
        timestamp: chrono::DateTime<chrono::Utc>,
        /// TTL for the message
        ttl: Option<String>,
        /// Sender information
        sender: BecknEndpoint,
        /// Receiver information
        receiver: Option<BecknEndpoint>,
    }

    /// Beckn endpoint description
    pub struct BecknEndpoint {
        /// Subscriber ID
        subscriber_id: String,
        /// Unique ID
        unique_id: Option<String>,
        /// Endpoint URI
        uri: Option<String>,
        /// Type of endpoint
        endpoint_type: Option<String>,
    }

    /// Base transaction state shared by all Beckn transactions
    pub trait BecknTransactionState: serde::Serialize + serde::de::DeserializeOwned {
        /// Get the transaction ID
        fn transaction_id(&self) -> &str;
        
        /// Get the current status
        fn status(&self) -> &TransactionStatus;
        
        /// Update the status
        fn update_status(&mut self, status: TransactionStatus);
        
        /// Get the domain this transaction belongs to
        fn domain(&self) -> &str;
        
        /// Get when this transaction was created
        fn created_at(&self) -> chrono::DateTime<chrono::Utc>;
        
        /// Get when this transaction was last updated
        fn updated_at(&self) -> chrono::DateTime<chrono::Utc>;
        
        /// Update the last updated timestamp
        fn touch(&mut self);
    }

    /// Common transaction status values
    pub enum TransactionStatus {
        Created,
        Acknowledged,
        Processing,
        Completed,
        Cancelled,
        Failed,
        // Domain-specific statuses can be added
    }

    /// Change to transaction state after handling an action
    pub struct StateChange {
        /// The updated state
        updated_state: Box<dyn BecknTransactionState>,
        /// Response message to send
        response: Option<serde_json::Value>,
        /// Whether to send a callback after processing
        send_callback: bool,
        /// The callback action if any
        callback_action: Option<BecknAction>,
    }

    /// Service handler trait for domain-specific logic
    pub trait ServiceHandler: Send + Sync {
        /// The type representing state for this service domain
        type State: BecknTransactionState;
        
        /// Get the domain this handler supports
        fn domain(&self) -> &str;
        
        /// Handle a Beckn action
        async fn handle_action(
            &self, 
            action: BecknAction, 
            context: ActionContext
        ) -> Result<StateChange, ServiceError>;
        
        /// Validate the payload for an action
        fn validate_payload(
            &self, 
            action: BecknAction, 
            payload: &serde_json::Value
        ) -> Result<(), ValidationError>;
        
        /// Initialize a new transaction
        fn initialize_transaction(
            &self, 
            transaction_id: &str,
            message: &serde_json::Value
        ) -> Result<Self::State, ServiceError>;
    }

    /// Main adapter for Beckn protocol operations
    pub struct BecknAdapter<S: ServiceHandler> {
        /// Domain-specific service handler
        service_handler: S,
        /// Identity for this node
        identity: std::sync::Arc<Identity>,
        /// Network manager for communication
        network: std::sync::Arc<NetworkManager>,
        /// Data store
        store: std::sync::Arc<WillowSledStore>,
        /// Access control manager
        access: std::sync::Arc<AccessManager>,
        /// Audit log sender
        audit: tokio::sync::mpsc::Sender<AuditEvent>,
        /// Configuration
        config: BecknAdapterConfig,
    }

    /// Configuration for the Beckn adapter
    pub struct BecknAdapterConfig {
        /// API port to listen on
        api_port: u16,
        /// Callback URL for this node
        callback_url: String,
        /// Registry configuration
        registry: RegistryConfig,
        /// Validation settings
        validation: ValidationConfig,
        /// Timeouts for different operations
        timeouts: TimeoutConfig,
    }
    ```
    *   **Core Functions**:
    ```rust
    impl<S: ServiceHandler> BecknAdapter<S> {
        /// Create a new Beckn adapter
        pub fn new(
            service_handler: S,
            identity: std::sync::Arc<Identity>,
            network: std::sync::Arc<NetworkManager>,
            store: std::sync::Arc<WillowSledStore>,
            access: std::sync::Arc<AccessManager>,
            audit: tokio::sync::mpsc::Sender<AuditEvent>,
            config: BecknAdapterConfig
        ) -> Self { ... }
        
        /// Start the adapter (begin listening for requests)
        pub async fn start(&self) -> Result<(), ServiceError> { ... }
        
        /// Stop the adapter
        pub async fn stop(&self) -> Result<(), ServiceError> { ... }
        
        /// Handle an incoming Beckn request
        pub async fn handle_request(
            &self,
            action: BecknAction,
            body: serde_json::Value
        ) -> Result<serde_json::Value, ServiceError> { ... }
        
        /// Send a Beckn request to another node
        pub async fn send_request(
            &self,
            receiver: &BecknEndpoint,
            action: BecknAction,
            message: serde_json::Value
        ) -> Result<serde_json::Value, ServiceError> { ... }
        
        /// Register this node with a Beckn registry
        pub async fn register_with_registry(
            &self,
            registry_url: &str
        ) -> Result<RegistrationResponse, ServiceError> { ... }
        
        /// Look up a provider in the registry
        pub async fn lookup_provider(
            &self,
            domain: &str,
            provider_id: &str
        ) -> Result<Vec<BecknEndpoint>, ServiceError> { ... }
    }
    ```
    *   **Registry Client**:
    ```rust
    pub mod registry {
        /// Register an identity with a Beckn registry
        pub async fn register_identity(
            net: &NetworkManager,
            id: &Identity,
            registry_url: &str
        ) -> Result<RegistrationResponse, RegistryError> { ... }
        
        /// Look up providers by role and domain
        pub async fn lookup_role(
            net: &NetworkManager,
            role: &str,
            domain: &str,
            registry_url: &str
        ) -> Result<Vec<ProviderInfo>, RegistryError> { ... }
        
        /// Update registration information
        pub async fn update_registration(
            net: &NetworkManager,
            id: &Identity,
            registry_url: &str,
            updates: RegistrationUpdates
        ) -> Result<UpdateResponse, RegistryError> { ... }
    }
    ```
    *   **Transaction Logic**:
    ```rust
    pub mod logic {
        /// Load transaction state from the store
        pub async fn load_state<T: BecknTransactionState>(
            store: &impl willow_data_model::Store,
            domain: &str,
            transaction_id: &str,
            subspace: &willow_25::SubspaceId25
        ) -> Result<T, LogicError> { ... }
        
        /// Save transaction state to the store
        pub async fn save_state<T: BecknTransactionState>(
            store: &impl willow_data_model::Store,
            state: &T,
            subspace: &willow_25::SubspaceId25,
            auth_token: &meadowcap::McAuthorisationToken<...>
        ) -> Result<(), LogicError> { ... }
        
        /// List transactions by domain and status
        pub async fn list_transactions(
            store: &impl willow_data_model::Store,
            domain: &str,
            status: Option<TransactionStatus>,
            subspace: &willow_25::SubspaceId25,
            limit: usize
        ) -> Result<Vec<TransactionSummary>, LogicError> { ... }
    }
    ```

## 3. Tech Stack

*   **Primary Language**: Rust (Stable toolchain)
*   **Core Crates**:
    *   `tokio`: Asynchronous runtime.
    *   `iroh` (v0.34.1): Networking (`iroh::net`), cryptography (`iroh::crypto`).
    *   `willow-data-model` (v0.2.0): Core Willow traits (`Store`, `NamespaceId`, etc.) and structures (`Path`, `Entry`).
    *   `meadowcap` (v0.2.0): Meadowcap capability implementation (`McCapability`, `McAuthorisationToken`).
    *   `willow_25` (v0.1.0): Concrete Willow/Meadowcap parameters (Ed25519, Blake3 based types like `SubspaceId25`, `Signature25`, `PayloadDigest25`).
    *   `iroh-willow`: Willow General Purpose Sync (WGPS) protocol implementation over Iroh (Likely part of `iroh` crate v0.34.1).
    *   `willow-store-simple-sled` (v0.1.0): Initial `Store` trait implementation using `sled`.
    *   `sled` (v0.34.7): Embedded key-value store backend.
    *   `serde`, `serde_json`: Data serialization/deserialization.
    *   `sqlx` (v0.8.5): For interacting with the PostgreSQL `AuditDB`.
    *   `tracing`: Logging and diagnostics framework.
    *   `thiserror`: Error handling boilerplate.
    *   `tracing-subscriber`: Logging subscriber implementation.
*   **External Dependencies**:
    *   PostgreSQL: For relational audit logs (`AuditDB`).

## 4. Data Structures & Persistence (Willow/Sled & PostgreSQL)

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

## 5. Networking Layer (Iroh via `net` module)

*   **Core**: `iroh::net::Endpoint` manages connections and network state.
*   **Identity**: Each instance runs an Endpoint identified by a `iroh::net::NodeId` (derived from an Ed25519 keypair, linked to `identity::Identity`).
*   **Discovery & Connection**:
    *   Direct connection via `endpoint.connect(node_id, addrs)` if peer details are known (e.g., from registry).
    *   Ticket-based connection (`iroh::net::Ticket`) for easy out-of-band sharing.
    *   Potential use of future Iroh discovery mechanisms if needed.
*   **Transport**: Secure, multiplexed connections via QUIC (provided by Iroh). Includes NAT traversal capabilities.
*   **Synchronization**: `iroh-willow` leverages Iroh connections to run the Willow General Purpose Sync (WGPS) protocol, synchronizing `Entry` data within specified `NamespaceId`s based on peer interests and capabilities, using the `store::WillowSledStore` as the data source/sink.

## 6. Security Implementation

*   **Node Authentication**: Handled natively by Iroh. Connections between nodes are authenticated using the Ed25519 keys underlying their `NodeId`.
*   **Data Access Control (Willow)**: Enforced by `meadowcap` capabilities. Writes to the `store` require a valid `McAuthorisationToken` signed by an identity holding an appropriate `McCapability` for the target `Path` and `SubspaceId`. Verification occurs implicitly when adding `AuthorisedEntry` data. Managed conceptually by the `access` module.
*   **Beckn Message Security**:
    *   Signatures: Beckn actions requiring signatures (as per spec) will be signed by the `identity::Identity`'s Beckn keypair.
    *   Verification: Incoming signed messages will be verified against the claimed sender's public key (potentially retrieved via the `store` or registry lookup).
*   **Transport Security**: Provided by Iroh using QUIC/TLS, securing data in transit between nodes.
*   **Audit Trail**: The `AuditDB` provides an immutable log for security monitoring.

## 7. Development Workflow & Testing

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

### 7.1 Unit Testing Examples

```rust
#[cfg(test)]
mod identity_tests {
    use super::*;
    
    #[test]
    fn test_identity_creation() {
        let identity = Identity::new(
            "Test Identity".to_string(),
            BecknRole::BAP,
            vec!["retail".to_string()]
        );
        
        assert_eq!(identity.metadata.name, "Test Identity");
        assert_eq!(identity.metadata.beckn_role, BecknRole::BAP);
        assert!(identity.metadata.domains.contains(&"retail".to_string()));
        assert!(!identity.did.is_empty());
    }
    
    #[test]
    fn test_identity_signing_and_verification() {
        let identity = Identity::new(
            "Test Identity".to_string(),
            BecknRole::BAP,
            vec!["retail".to_string()]
        );
        
        let data = b"Test data to sign";
        let signature = identity.sign(data);
        
        assert!(identity.verify(data, &signature));
        
        // Modify data and ensure verification fails
        let modified_data = b"Modified test data";
        assert!(!identity.verify(modified_data, &signature));
    }
    
    #[test]
    fn test_beckn_auth_header_generation() {
        let identity = Identity::new(
            "Test Identity".to_string(),
            BecknRole::BAP,
            vec!["retail".to_string()]
        );
        
        let request_body = r#"{"context":{"domain":"retail"},"message":{"value":"test"}}"#;
        let auth_header = identity.create_auth_header(request_body);
        
        // Verify header format: Signature keyId="...",algorithm="ed25519",created="...",expires="...",headers="...",signature="..."
        assert!(auth_header.starts_with("Signature keyId="));
        assert!(auth_header.contains("algorithm=\"ed25519\""));
        assert!(auth_header.contains("signature=\""));
    }
}

#[cfg(test)]
mod access_tests {
    use super::*;
    use mockall::predicate::*;
    
    #[tokio::test]
    async fn test_capability_granting() {
        // Set up mocks
        let mut mock_store = MockWillowSledStore::new();
        let identity = std::sync::Arc::new(
            Identity::new(
                "Grantor".to_string(),
                BecknRole::BAP,
                vec!["retail".to_string()]
            )
        );
        
        let grantee_identity = Identity::new(
            "Grantee".to_string(),
            BecknRole::BPP,
            vec!["retail".to_string()]
        );
        
        // Configure mock expectations
        mock_store.expect_put()
            .returning(|_, _, _, _| {
                Ok(willow_data_model::Entry {
                    timestamp: willow_data_model::Timestamp::now(),
                    payload_digest: willow_25::PayloadDigest25::from_bytes(&[0; 32]),
                    payload: vec![],
                })
            });
        
        let access_manager = AccessManager::new(
            identity.clone(),
            std::sync::Arc::new(mock_store)
        );
        
        // Create a capability grant
        let grant = CapabilityGrant {
            grantee_did: grantee_identity.as_did_string().to_string(),
            subspace: identity.subspace_id(),
            path_pattern: PathPattern {
                pattern: "/transactions/retail/*".to_string(),
                is_pattern: true,
            },
            operations: vec![Operation::Read, Operation::Write],
            expires_at: None,
            delegatable: false,
        };
        
        // Grant the capability
        let result = access_manager.grant_capability(grant).await;
        
        assert!(result.is_ok());
        let capability = result.unwrap();
        
        // Verify the capability properties
        assert_eq!(capability.metadata.grantor_did, identity.as_did_string());
        assert_eq!(capability.metadata.grantee_did, grantee_identity.as_did_string());
    }
}
```

### 7.2 Integration Testing Examples

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_store_with_access_control() {
        // Set up test environment
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_path_buf();
        
        let store_config = StoreConfig {
            db_path,
            cache_size_bytes: 1024 * 1024,
            flush_every_write: true,
            compaction: CompactionConfig::default(),
            backup: BackupConfig::default(),
        };
        
        let store = std::sync::Arc::new(
            WillowSledStore::new(store_config, None).unwrap()
        );
        
        let identity = std::sync::Arc::new(
            Identity::new(
                "Test Identity".to_string(),
                BecknRole::BAP,
                vec!["retail".to_string()]
            )
        );
        
        let access_manager = std::sync::Arc::new(
            AccessManager::new(identity.clone(), store.clone())
        );
        
        // Test writing with access control
        let path = WillowSledStore::build_path(&["test", "data"]).unwrap();
        let data = serde_json::json!({"test": "value"});
        
        // Create capability for writing
        let grant = CapabilityGrant {
            grantee_did: identity.as_did_string().to_string(),
            subspace: identity.subspace_id(),
            path_pattern: PathPattern {
                pattern: "/test/*".to_string(),
                is_pattern: true,
            },
            operations: vec![Operation::Write],
            expires_at: None,
            delegatable: false,
        };
        
        let capability = access_manager.grant_capability(grant).await.unwrap();
        
        let auth_token = access_manager.create_auth_token(
            &capability.capability,
            &path,
            Operation::Write
        ).await.unwrap();
        
        // Write data with authorization
        let payload = serde_json::to_vec(&data).unwrap();
        let result = store.put(
            &identity.subspace_id(),
            &path,
            payload.into(),
            Some(auth_token)
        ).await;
        
        assert!(result.is_ok());
        
        // Read data back
        let entry_result = store.get_with_payload(
            &identity.subspace_id(),
            &path
        ).await;
        
        assert!(entry_result.is_ok());
        let (_, payload_data) = entry_result.unwrap().unwrap();
        let read_data: serde_json::Value = serde_json::from_slice(&payload_data).unwrap();
        
        assert_eq!(read_data, data);
    }
    
    #[tokio::test]
    async fn test_service_adapter_with_store_and_network() {
        // Set up test environment with temporary store
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_path_buf();
        
        // Create identities
        let bap_identity = std::sync::Arc::new(
            Identity::new(
                "BAP Identity".to_string(), 
                BecknRole::BAP,
                vec!["retail".to_string()]
            )
        );
        
        let bpp_identity = std::sync::Arc::new(
            Identity::new(
                "BPP Identity".to_string(), 
                BecknRole::BPP,
                vec!["retail".to_string()]
            )
        );
        
        // Create store
        let store_config = StoreConfig {
            db_path,
            cache_size_bytes: 1024 * 1024,
            flush_every_write: true,
            compaction: CompactionConfig::default(),
            backup: BackupConfig::default(),
        };
        
        let store = std::sync::Arc::new(
            WillowSledStore::new(store_config, None).unwrap()
        );
        
        // Create access manager
        let access_manager = std::sync::Arc::new(
            AccessManager::new(bap_identity.clone(), store.clone())
        );
        
        // Create network manager (with mock configuration)
        let network_config = NetworkConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            known_peers: vec![],
            sync_interval_seconds: 60,
            connection_timeout_seconds: 10,
            max_concurrent_connections: 10,
        };
        
        let network = std::sync::Arc::new(
            NetworkManager::new(bap_identity.as_ref(), network_config).await.unwrap()
        );
        
        // Create audit channel
        let (audit_sender, _audit_receiver) = tokio::sync::mpsc::channel(100);
        
        // Create the retail service handler
        let retail_handler = RetailServiceHandler::new();
        
        // Create the Beckn adapter
        let adapter_config = BecknAdapterConfig {
            api_port: 0, // Use any available port
            callback_url: "http://localhost/callback".to_string(),
            registry: RegistryConfig::default(),
            validation: ValidationConfig::default(),
            timeouts: TimeoutConfig::default(),
        };
        
        let adapter = BecknAdapter::new(
            retail_handler,
            bap_identity.clone(),
            network.clone(),
            store.clone(),
            access_manager.clone(),
            audit_sender,
            adapter_config
        );
        
        // Test a search request
        let search_request = serde_json::json!({
            "context": {
                "domain": "retail",
                "action": "search",
                "transaction_id": "test-txn-123",
                "message_id": "msg-123",
                "timestamp": "2023-01-01T12:00:00Z"
            },
            "message": {
                "intent": {
                    "item": {
                        "descriptor": {
                            "name": "Test Product"
                        }
                    }
                }
            }
        });
        
        let response = adapter.handle_request(
            BecknAction::Search,
            search_request
        ).await;
        
        assert!(response.is_ok());
        
        // Verify that a transaction was created in the store
        let state_result = logic::load_state::<RetailTransactionState>(
            store.as_ref(),
            "retail",
            "test-txn-123",
            &bap_identity.subspace_id()
        ).await;
        
        assert!(state_result.is_ok());
        let state = state_result.unwrap();
        
        assert_eq!(state.transaction_id(), "test-txn-123");
        assert_eq!(state.domain(), "retail");
    }
}
```

### 7.3 Beckn Protocol Conformance Testing

```rust
#[cfg(test)]
mod beckn_protocol_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_search_flow() {
        // Set up the test environment
        let test_env = TestEnvironment::new().await;
        
        // Create a search request payload
        let search_payload = serde_json::json!({
            "context": {
                "domain": "retail",
                "country": "IND",
                "city": "std:080",
                "action": "search",
                "timestamp": "2023-01-01T12:00:00Z",
                "message_id": "abc-123",
                "transaction_id": "txn-123",
                "bap_id": "example-bap.com",
                "bap_uri": "https://example-bap.com"
            },
            "message": {
                "intent": {
                    "item": {
                        "descriptor": {
                            "name": "Smartphone"
                        },
                        "price": {
                            "currency": "INR",
                            "range": {
                                "min": 10000,
                                "max": 50000
                            }
                        }
                    }
                }
            }
        });
        
        // Send the search request from BAP to BPP
        let search_response = test_env.bap_adapter.handle_request(
            BecknAction::Search,
            search_payload
        ).await.unwrap();
        
        // Verify response format conforms to Beckn protocol
        assert!(search_response.get("context").is_some());
        assert_eq!(
            search_response["context"]["action"].as_str().unwrap(),
            "search"
        );
        
        // Check ack/nack format
        assert!(search_response.get("message").is_some());
        assert!(search_response["message"].get("ack").is_some());
        
        // Verify transaction state was created
        let txn_state = test_env.get_transaction_state("txn-123").await.unwrap();
        assert_eq!(txn_state.transaction_id(), "txn-123");
        assert_eq!(txn_state.status(), &TransactionStatus::Acknowledged);
        
        // Simulate an on_search callback response
        let on_search_payload = serde_json::json!({
            "context": {
                "domain": "retail",
                "country": "IND",
                "city": "std:080",
                "action": "on_search",
                "timestamp": "2023-01-01T12:01:00Z",
                "message_id": "def-456",
                "transaction_id": "txn-123",
                "bpp_id": "example-bpp.com",
                "bpp_uri": "https://example-bpp.com"
            },
            "message": {
                "catalog": {
                    "providers": [
                        {
                            "id": "provider-1",
                            "descriptor": {
                                "name": "Example Electronics"
                            },
                            "items": [
                                {
                                    "id": "item-1",
                                    "descriptor": {
                                        "name": "Smartphone X"
                                    },
                                    "price": {
                                        "currency": "INR",
                                        "value": "25000"
                                    }
                                }
                            ]
                        }
                    ]
                }
            }
        });
        
        // Process the on_search callback
        let callback_response = test_env.bap_adapter.handle_request(
            BecknAction::OnSearch,
            on_search_payload
        ).await.unwrap();
        
        // Verify callback was processed correctly
        assert!(callback_response.get("message").is_some());
        assert!(callback_response["message"].get("ack").is_some());
        
        // Verify transaction state was updated with catalog data
        let updated_txn_state = test_env.get_transaction_state("txn-123").await.unwrap();
        assert_eq!(updated_txn_state.transaction_id(), "txn-123");
        assert_eq!(updated_txn_state.status(), &TransactionStatus::Completed);
        
        // Check that the catalog data was stored
        let retail_state = updated_txn_state.as_retail_state();
        assert!(retail_state.catalog.is_some());
        assert_eq!(retail_state.catalog.as_ref().unwrap().providers.len(), 1);
        assert_eq!(
            retail_state.catalog.as_ref().unwrap().providers[0].items.len(),
            1
        );
    }
}
```

## 8. Monitoring & Observability

The Chiti Identity system provides comprehensive monitoring capabilities to ensure reliability, performance, and security.

### 8.1 Health Checks

* **HTTP Endpoints**:
  * `/health`: Basic system health status
  * `/health/detailed`: Component-level health status
  * `/health/store`: Store connectivity and status
  * `/health/network`: Network connectivity status
  * `/health/registry`: Registry connectivity check

* **Self-Diagnostics**:
  * Automatic periodic self-tests
  * Resource checks (disk space, memory usage)
  * Certificate/key expiration monitoring
  * Connection pool status

### 8.2 Metrics

The system exports metrics in Prometheus format:

* **System Metrics**:
  * CPU/memory/disk usage
  * Open file descriptors
  * Garbage collection metrics

* **Store Metrics**:
  * Operations per second (reads/writes)
  * Operation latency (p50, p95, p99)
  * Entry count by subspace
  * Store size
  * Sync progress

* **Network Metrics**:
  * Active connections
  * Connection attempts (success/failure)
  * Data transfer rates
  * Connection latency

* **Beckn Protocol Metrics**:
  * Requests per second by action type
  * Response times
  * Error rates
  * Transaction counts by domain and status

* **Security Metrics**:
  * Authorization attempts (success/failure)
  * Token creation rate
  * Capability grants
  * Signature verification time

### 8.3 Logging

* **Structured Logging**:
  * JSON format with standard fields
  * Correlation IDs across components
  * Context-aware logging with tracing spans

* **Log Levels**:
  * ERROR: Critical issues requiring immediate attention
  * WARN: Potential issues that should be investigated
  * INFO: Normal operation events (default)
  * DEBUG: Detailed information for troubleshooting
  * TRACE: Very detailed debugging information

* **Log Categories**:
  * System: Core system events
  * Network: Connectivity and sync events
  * Store: Data operations
  * Security: Authentication and authorization events
  * Protocol: Beckn protocol interactions

* **Log Management**:
  * Log rotation and compression
  * External log forwarding (ELK, Splunk)
  * Audit log separation

### 8.4 Alerting

* **Alert Triggers**:
  * Service unavailability
  * High error rates
  * Resource exhaustion (disk, memory)
  * Security incidents
  * Performance degradation

* **Alert Channels**:
  * Email notifications
  * Webhook integration (Slack, Discord)
  * SMS/mobile notifications
  * PagerDuty integration

* **Alert Severity Levels**:
  * Critical: Requires immediate action
  * Major: Significant impact, quick response needed
  * Minor: Limited impact, should be addressed
  * Warning: Potential future issue

## 9. Deployment & Operations

### 9.1 Deployment Architectures

The system supports multiple deployment models:

#### Standalone Deployment

* Single instance running all components
* Suitable for development, testing, or small-scale deployments
* Simplified management but limited scalability and redundancy

```
┌────────────────────────────────────────┐
│           Single Node Instance         │
│                                        │
│  ┌─────────┐  ┌─────────┐  ┌────────┐  │
│  │ Identity│  │ Service │  │  API   │  │
│  │ + Store │  │ Adapter │  │        │  │
│  └─────────┘  └─────────┘  └────────┘  │
│                                        │
│  ┌────────────────┐  ┌──────────────┐  │
│  │ Sled Database  │  │ PostgreSQL   │  │
│  │ (Willow Store) │  │ (Audit Log)  │  │
│  └────────────────┘  └──────────────┘  │
└────────────────────────────────────────┘
```

#### Distributed Deployment

* Multiple instances across different machines
* Each instance has its own Willow store
* Synchronization via Iroh network
* Shared PostgreSQL cluster for audit logs
* Improved scalability and redundancy

```
┌─────────────────────┐  ┌─────────────────────┐
│     BAP Instance    │  │     BPP Instance    │
│  ┌────────┐ ┌─────┐ │  │  ┌────────┐ ┌─────┐ │
│  │Identity│ │Store│ │  │  │Identity│ │Store│ │
│  └────────┘ └─────┘ │  │  └────────┘ └─────┘ │
│  ┌────────────────┐ │  │  ┌────────────────┐ │
│  │BAP ServiceAdapter│===│  │BPP ServiceAdapter│
│  └────────────────┘ │  │  └────────────────┘ │
└─────────────────────┘  └─────────────────────┘
           │                        │
           │                        │
           ▼                        ▼
  ┌──────────────────────────────────────────┐
  │           Shared PostgreSQL              │
  │          (Audit Database)                │
  └──────────────────────────────────────────┘
```

#### Containerized Deployment

* Each component in separate containers
* Orchestrated with Docker Compose or Kubernetes
* Configuration via environment variables and config maps
* Highest flexibility and scalability

```
┌─────────────────────────────────────────────┐
│               Kubernetes Cluster            │
│                                             │
│  ┌─────────┐  ┌─────────┐  ┌─────────────┐  │
│  │ Identity│  │ Network │  │  Service    │  │
│  │ Pods    │  │ Pods    │  │  Adapter    │  │
│  │         │  │         │  │  Pods       │  │
│  └─────────┘  └─────────┘  └─────────────┘  │
│                                             │
│  ┌─────────────────┐    ┌───────────────┐   │
│  │ Store Pods      │    │ PostgreSQL    │   │
│  │ (StatefulSet)   │    │ (StatefulSet) │   │
│  └─────────────────┘    └───────────────┘   │
│                                             │
│  ┌─────────────────┐    ┌───────────────┐   │
│  │ Config Maps     │    │ Secrets       │   │
│  └─────────────────┘    └───────────────┘   │
└─────────────────────────────────────────────┘
```

### 9.2 Configuration Management

Configuration is managed through a layered approach:

1. **Default Configuration**: Hardcoded sensible defaults
2. **Configuration Files**: TOML or YAML files for static configuration
3. **Environment Variables**: Override specific settings
4. **Runtime Configuration**: Stored in the Willow store, modifiable via API

Example configuration file:

```toml
[identity]
key_storage_path = "/opt/chiti-identity/keys"
default_role = "BAP"
default_domains = ["retail", "mobility"]

[store]
db_path = "/var/lib/chiti-identity/data"
cache_size_bytes = 104857600  # 100 MB
flush_every_write = true
namespace = "chiti-identity"  # Optional fixed namespace

[network]
listen_addr = "0.0.0.0:4433"
connection_timeout_seconds = 30
max_concurrent_connections = 100
known_peers = [
  { node_id = "12D3KooWAbcdef...", addresses = ["1.2.3.4:4433"], auto_connect = true }
]

[audit]
database_url = "postgresql://user:password@localhost/audit"
retention_days = 90
sync_writes = true

[beckn]
api_port = 8080
callback_url = "https://example.com/beckn/callback"
registry_url = "https://registry.becknprotocol.io/v1"
validation_level = "strict"  # strict, lenient
timeouts = { request_seconds = 30, callback_seconds = 120 }
```

Environment variables can override these settings:

```
CHITI_IDENTITY_KEY_STORAGE_PATH=/custom/path
CHITI_STORE_DB_PATH=/custom/db/path
CHITI_NETWORK_LISTEN_ADDR=0.0.0.0:9000
CHITI_AUDIT_DATABASE_URL=postgresql://user:pass@db.example.com/audit
CHITI_BECKN_API_PORT=9080
```

### 9.3 Backup & Recovery

The system implements a robust backup strategy:

#### Automated Backups

* **Willow Store**: 
  * Scheduled incremental backups
  * Retention policy management
  * Compression and optional encryption
  * Point-in-time recovery capability

* **PostgreSQL Audit Database**:
  * Standard PostgreSQL backup procedures (pg_dump)
  * Optional streaming replication for high availability
  * WAL archiving for point-in-time recovery

* **Cryptographic Material**:
  * Secure backup of identity keys
  * Hardware security module integration (optional)
  * Encrypted export format

#### Recovery Procedures

1. **Willow Store Recovery**:
   ```rust
   // Example recovery code
   let store = WillowSledStore::new(config, None)?;
   store.restore_from_backup("/path/to/backup/file").await?;
   ```

2. **Full System Recovery**:
   * Restore identity keys
   * Restore Willow store from backup
   * Restore or reconnect to PostgreSQL
   * Resume network synchronization

3. **Disaster Recovery**:
   * Deploy new instance from scratch
   * Restore identity and configuration
   * Import data backup
   * Re-establish network connections

### 9.4 Upgrade Strategy

* **In-place Upgrades**:
  * Stop service
  * Replace binary
  * Run migrations if needed
  * Start service

* **Blue-Green Deployment**:
  * Deploy new version alongside existing
  * Verify functionality
  * Switch traffic to new version
  * Decommission old version

* **Database Migrations**:
  * Schema version tracking
  * Automated migration scripts
  * Rollback capability
  * Store compaction after significant changes

## 10. Future Considerations

### 10.1 Alternative `Store` Implementations

The initial implementation uses `willow-store-simple-sled` for its simplicity and embedded nature. Future versions could explore alternative backends:

* **RocksDB Backend**: A high-performance embedded database with better handling of large datasets.
  ```rust
  pub struct WillowRocksDBStore {
      // RocksDB implementation of Willow Store
  }
  ```

* **PostgreSQL Backend**: For integration with existing database infrastructure.
  ```rust
  pub struct WillowPostgresStore {
      // PostgreSQL implementation of Willow Store
      pool: sqlx::PgPool,
  }
  ```

* **Distributed Store**: A multi-node replicated store for high availability.
  ```rust
  pub struct WillowDistributedStore {
      // Distributed implementation with consensus protocol
  }
  ```

### 10.2 Advanced Cryptography & Security

Future security enhancements may include:

* **Payload Encryption**: End-to-end encryption of payloads stored in the Willow system.
  ```rust
  pub struct EncryptedPayload {
      ciphertext: Vec<u8>,
      nonce: [u8; 12],
      recipient_id: willow_25::SubspaceId25,
  }
  
  impl PayloadProcessor for E2EEncryptionProcessor {
      fn process_outgoing(&self, payload: Vec<u8>, recipients: &[willow_25::SubspaceId25]) -> Vec<u8> {
          // Encrypt payload for recipients
      }
      
      fn process_incoming(&self, payload: Vec<u8>) -> Option<Vec<u8>> {
          // Decrypt payload if we are a recipient
      }
  }
  ```

* **Advanced Capability Management**:
  * Capability revocation protocols
  * Delegated capabilities with restriction
  * Time-bound and usage-limited capabilities
  * Capability disclosure minimization

* **Hardware Security Modules**: Integration with HSMs for key protection.
  ```rust
  pub struct HsmIdentityProvider {
      // Implementation using HSM for cryptographic operations
  }
  ```

* **Quantum-Resistant Cryptography**: Preparing for the post-quantum era.
  ```rust
  pub mod willow_pq {
      // Post-quantum versions of cryptographic primitives
  }
  ```

### 10.3 Enhanced Beckn Protocol Support

* **Additional Service Domains**:
  * Mobility
  * Logistics
  * Healthcare
  * Agriculture
  * Education
  * Professional services

* **Beckn Gateway Role**: Support for acting as a Beckn Gateway.
  ```rust
  pub struct BecknGateway {
      // Gateway implementation connecting BAPs and BPPs
  }
  ```

* **Multi-Protocol Adapters**: Support for multiple protocol versions.
  ```rust
  pub enum BecknProtocolVersion {
      V0_9_3,
      V1_0_0,
      V1_1_0,
  }
  ```

* **Enhanced Schema Validation**: Stricter validation with custom rules.
  ```rust
  pub struct EnhancedValidator {
      // Enhanced validation with business rule enforcement
  }
  ```

### 10.4 Scalability Improvements

* **Hierarchical Synchronization**: More efficient sync strategies.
  ```rust
  pub struct HierarchicalSyncManager {
      // Manages sync with hierarchical segmentation
  }
  ```

* **Selective Replication**: Replicating only needed data.
  ```rust
  pub struct SelectiveReplicationPolicy {
      // Defines what data gets replicated to which nodes
  }
  ```

* **Sharding Support**: Horizontal scaling through sharding.
  ```rust
  pub struct ShardManager {
      // Manages distribution of identities across shards
  }
  ```

* **Improved Caching**: Multi-level caching for better performance.
  ```rust
  pub struct MultiLevelCache {
      // L1/L2 cache implementation for data access
  }
  ```

### 10.5 Developer Experience

* **SDK Libraries**: Language-specific SDKs for Chiti Identity integration.
  ```
  chiti-js           // JavaScript/TypeScript SDK
  chiti-python       // Python SDK
  chiti-java         // Java SDK
  ```

* **Admin Dashboard**: Web-based administration interface.
  ```
  chiti-admin-ui     // React-based administration interface
  ```

* **CLI Tools**: Enhanced command-line tools for management.
  ```
  chiti-cli          // Command-line interface for identity management
  ```

* **Testing Framework**: Specialized testing tools for Beckn interactions.
  ```
  chiti-test-suite   // Testing framework for Beckn protocol compliance
  ```

### 10.6 Integration & Interoperability

* **Standards Compliance**: Alignment with additional identity standards.
  * W3C DID improvements
  * Verifiable Credentials support
  * Integration with existing identity providers

* **Cross-Protocol Bridges**: Integration with other identity ecosystems.
  ```rust
  pub mod bridges {
      pub struct EthereumIdentityBridge;
      pub struct IndyCredentialBridge;
      pub struct OpenIdConnectBridge;
  }
  ```

* **Ecosystem Connectors**: Ready-made integrations with external systems.
  ```rust
  pub mod connectors {
      pub struct PaymentConnector;
      pub struct AnalyticsConnector;
      pub struct ComplianceConnector;
  }
  ```

### 10.7 Advanced Analytics & Insights

* **Transaction Analytics**: Insights from Beckn transactions.
  ```rust
  pub struct TransactionAnalytics {
      // Analyzes transaction patterns and performance
  }
  ```

* **Network Health Monitoring**: Proactive monitoring of network.
  ```rust
  pub struct NetworkHealthMonitor {
      // Monitors and reports on network health metrics
  }
  ```

* **Semantic Insights**: Extracting meaning from transaction data.
  ```rust
  pub struct SemanticAnalyzer {
      // Extracts insights from transaction semantics
  }
  ```

* **Performance Benchmarking**: Automated performance testing.
  ```rust
  pub struct BenchmarkSuite {
      // Runs performance benchmarks automatically
  }
  ```

By pursuing these future directions, the Chiti Identity system can evolve to meet emerging needs while maintaining its core principles of decentralization, security, and standards compliance.