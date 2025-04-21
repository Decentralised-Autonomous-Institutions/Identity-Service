# Service Adaptation Layer Architecture (`service_adapter`)

This document details the internal architecture of the Service Adaptation Layer, represented primarily by the `service_adapter` Rust module. Its core purpose is to handle the logic and interactions specific to the Beckn protocol, acting as the bridge between external Beckn network participants and the core identity, networking, and storage capabilities of the Chiti node.

A key design goal is **modularity and extensibility** to support various Beckn domains (retail, mobility, logistics, etc.) without requiring changes to the core adapter logic.

## 1. Core Design: Generic Adapter + Service Handler Trait

The `service_adapter` is designed as a generic component that takes a specific **Service Handler** as a parameter. This handler encapsulates all the logic unique to a particular Beckn service domain.

```rust
// Conceptual Representation

/// The main adapter struct, generic over a ServiceHandler implementation.
pub struct BecknAdapter<S: ServiceHandler> {
    service_handler: S,
    network_manager: Arc<NetworkManager>, // From 'net' module
    store: Arc<dyn Store + Send + Sync>, // From 'store' module (using Arc<dyn> for flexibility)
    access_manager: Arc<AccessManager>, // From 'access' module
    identity: Arc<Identity>, // From 'identity' module
    audit_svc: Arc<AuditService>, // From 'audit' module
    // ... other necessary fields
}

/// Trait defining the interface for a specific Beckn service implementation.
pub trait ServiceHandler: Send + Sync {
    /// Associated type representing the specific state managed by this service.
    type State: BecknTransactionState + serde::Serialize + serde::de::DeserializeOwned;

    /// Handles a specific Beckn action (e.g., "on_search", "on_select").
    /// Takes the current state (if applicable) and action context.
    /// Returns necessary state changes or actions (like sending a message).
    async fn handle_action(
        &self,
        action: BecknAction,
        context: ActionContext,
        current_state: Option<Self::State>,
    ) -> Result<ServiceOutput<Self::State>, ServiceError>;

    /// Validates the payload specific to this service and action.
    fn validate_payload(
        &self,
        action: BecknAction,
        payload: &serde_json::Value,
    ) -> Result<(), ValidationError>;
}

/// Represents the shared state across all Beckn transactions.
pub trait BecknTransactionState: Send + Sync {
    fn transaction_id(&self) -> &str;
    fn status(&self) -> &str; // Example shared status field
    // ... other common fields
}

// Example context and output types
pub struct ActionContext { /* ... details like sender NodeId, transaction_id, etc. */ }
pub enum ServiceOutput<S> { /* ... details like NewState(S), MessageToSend(Vec<u8>, NodeId), NoChange, etc. */ }
pub enum BecknAction { /* ... enum representing Beckn actions like Search, OnSearch, Select, OnInit, etc. */ }
pub struct ServiceError { /* ... error details */ }
pub struct ValidationError { /* ... validation error details */ }
```

## 2. Interaction Flow

A typical flow for handling an incoming Beckn message (e.g., an `on_search` request) would look like this:

1.  **Receive Message**: The `net` module receives data over Iroh.
2.  **Initial Parsing**: The `service_adapter` (or a layer above it) parses the message into a generic Beckn structure, identifying the action (e.g., `on_search`), sender, `transaction_id`, etc.
3.  **Schema Validation**: The `validation` sub-component within `service_adapter` performs initial validation against the standard Beckn OpenAPI schema for the given action.
4.  **Service-Specific Validation**: The adapter calls `ServiceHandler::validate_payload` on the plugged-in handler to perform domain-specific checks on the message payload.
5.  **Load State**: The `logic` sub-component interacts with the `store` module to load the current `ServiceHandler::State` for the given `transaction_id` (using appropriate Willow paths).
6.  **Execute Action Logic**: The adapter calls `ServiceHandler::handle_action`, passing the action details, context (sender info, etc.), and the loaded state.
7.  **Process Output**: The `ServiceHandler` returns a `ServiceOutput` indicating:
    *   **State Change**: If the state needs to be updated, the `logic` sub-component interacts with the `access` module to potentially generate necessary `McAuthorisationToken`s and then saves the new state to the `store`.
    *   **Message to Send**: If a response or a new message needs to be sent (e.g., `select` after `on_search`), the adapter uses the `net` module to send the data to the appropriate peer.
    *   **No Change / Error**: Handle accordingly.
8.  **Auditing**: Throughout the process, relevant events are sent to the `AUDIT_SVC`.

## 3. Sub-Components

While potentially implemented within the `service_adapter` module itself, these conceptual components handle specific tasks:

*   **`validation`**: Responsible for initial schema validation based on Beckn OpenAPI specifications. It delegates domain-specific payload validation to the `ServiceHandler`.
*   **`registry` Client**: Handles interactions with the Beckn Registry via the `net` module. Used for looking up participants (`lookup_role`) and potentially registering the node's own identity/roles.
*   **`logic` (Transaction Logic)**: Manages the loading and saving of `BecknTransactionState` (including the service-specific `ServiceHandler::State`) from/to the `store`. Ensures state transitions are persisted correctly using appropriate Willow paths and authorisation tokens.

## 4. Example: Retail Service Handler

```rust
// Conceptual implementation for a hypothetical Retail service

pub struct RetailServiceState {
    txn_id: String,
    common_status: String,
    // Retail-specific fields
    items: Vec<Item>,
    quote: Option<Quote>,
}

impl BecknTransactionState for RetailServiceState { /* ... impl methods ... */ }

pub struct RetailServiceHandler { /* ... configuration, external service clients, etc. */ }

impl ServiceHandler for RetailServiceHandler {
    type State = RetailServiceState;

    async fn handle_action(
        &self,
        action: BecknAction,
        context: ActionContext,
        current_state: Option<Self::State>,
    ) -> Result<ServiceOutput<Self::State>, ServiceError> {
        match action {
            BecknAction::OnSearch => { /* ... logic specific to handling on_search ... */ }
            BecknAction::OnSelect => { /* ... logic specific to handling on_select ... */ }
            // ... other retail actions
            _ => { /* ... default or error ... */ }
        }
        // ... determine state changes or messages to send
    }

    fn validate_payload(
        &self,
        action: BecknAction,
        payload: &serde_json::Value,
    ) -> Result<(), ValidationError> {
        match action {
            BecknAction::OnSearch => { /* ... validate retail on_search payload structure ... */ }
            // ... other retail actions
            _ => Ok(()), // Or specific validation
        }
    }
}

// In main application setup:
let retail_handler = RetailServiceHandler { /* ... */ };
let adapter = BecknAdapter::new(
    retail_handler, 
    network_manager, 
    store,
    access_manager,
    identity,
    audit_svc,
);
```

This structure allows the core `BecknAdapter` to remain stable while supporting diverse Beckn services simply by implementing the `ServiceHandler` trait for each required domain. 