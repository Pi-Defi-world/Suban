#![no_std]

//! Event Registry — unified cross-primitive event storage and querying.
//! Any contract can register events here for cross-primitive auditability.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventRecord {
    pub event_id: u64,
    pub primitive: Symbol,   // "amm", "lending", "escrow", "bridge", "oracle"
    pub event_type: Symbol,  // "swap", "deposit", "borrow", "milestone", etc.
    pub contract_address: Address,
    pub actor: Address,
    pub data: Symbol,        // serialized event data (JSON string or compact encoding)
    pub ledger: u32,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    EventCount,
    Event(u64),
    PrimitiveIndex(Symbol, u64),   // primitive → event_id
    ContractIndex(Address, u64),   // contract → event_id
    ActorIndex(Address, u64),      // actor → event_id
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    NotAdmin = 1,
    AlreadyInitialized = 2,
    EventNotFound = 3,
}

#[contract]
pub struct EventRegistry;

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

fn next_event_id(env: &Env) -> u64 {
    let count: u64 = env
        .storage()
        .instance()
        .get::<DataKey, u64>(&DataKey::EventCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::EventCount, &(count + 1));
    count
}

#[contractimpl]
impl EventRegistry {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::EventCount, &0u64);
    }

    /// Register an event from any primitive. Callable by any contract.
    pub fn register_event(
        env: Env,
        primitive: Symbol,
        event_type: Symbol,
        contract_address: Address,
        actor: Address,
        data: Symbol,
    ) -> u64 {
        let event_id = next_event_id(&env);
        let ledger = env.ledger().sequence();
        let timestamp = env.ledger().timestamp();

        let record = EventRecord {
            event_id,
            primitive: primitive.clone(),
            event_type,
            contract_address: contract_address.clone(),
            actor: actor.clone(),
            data,
            ledger,
            timestamp,
        };

        env.storage().instance().set(&DataKey::Event(event_id), &record);
        env.storage().instance().set(
            &DataKey::PrimitiveIndex(primitive, event_id),
            &event_id,
        );
        env.storage().instance().set(
            &DataKey::ContractIndex(contract_address, event_id),
            &event_id,
        );
        env.storage().instance().set(
            &DataKey::ActorIndex(actor, event_id),
            &event_id,
        );

        env.events().publish(
            (symbol_short!("evt_reg"), event_id),
            &record.primitive,
        );

        event_id
    }

    /// Get event by ID.
    pub fn get_event(env: Env, event_id: u64) -> Result<EventRecord, RegistryError> {
        env.storage()
            .instance()
            .get::<DataKey, EventRecord>(&DataKey::Event(event_id))
            .ok_or(RegistryError::EventNotFound)
    }

    /// Get total event count.
    pub fn event_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::EventCount)
            .unwrap_or(0)
    }

    /// List events by primitive type (e.g., "amm", "lending", "escrow").
    pub fn list_by_primitive(env: Env, primitive: Symbol, limit: u32) -> Vec<EventRecord> {
        let mut results = Vec::new(&env);
        let count: u64 = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::EventCount)
            .unwrap_or(0);

        let mut found = 0u32;
        let mut i = count;
        while i > 0 && found < limit {
            i -= 1;
            if let Some(record) = env
                .storage()
                .instance()
                .get::<DataKey, EventRecord>(&DataKey::Event(i))
            {
                if record.primitive == primitive {
                    results.push_back(record);
                    found += 1;
                }
            }
        }
        results
    }

    /// List events by contract address.
    pub fn list_by_contract(env: Env, contract_address: Address, limit: u32) -> Vec<EventRecord> {
        let mut results = Vec::new(&env);
        let count: u64 = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::EventCount)
            .unwrap_or(0);

        let mut found = 0u32;
        let mut i = count;
        while i > 0 && found < limit {
            i -= 1;
            if let Some(record) = env
                .storage()
                .instance()
                .get::<DataKey, EventRecord>(&DataKey::Event(i))
            {
                if record.contract_address == contract_address {
                    results.push_back(record);
                    found += 1;
                }
            }
        }
        results
    }

    /// List events by actor (user address).
    pub fn list_by_actor(env: Env, actor: Address, limit: u32) -> Vec<EventRecord> {
        let mut results = Vec::new(&env);
        let count: u64 = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::EventCount)
            .unwrap_or(0);

        let mut found = 0u32;
        let mut i = count;
        while i > 0 && found < limit {
            i -= 1;
            if let Some(record) = env
                .storage()
                .instance()
                .get::<DataKey, EventRecord>(&DataKey::Event(i))
            {
                if record.actor == actor {
                    results.push_back(record);
                    found += 1;
                }
            }
        }
        results
    }

    /// List recent events (global).
    pub fn list_recent(env: Env, limit: u32) -> Vec<EventRecord> {
        let mut results = Vec::new(&env);
        let count: u64 = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::EventCount)
            .unwrap_or(0);

        let mut found = 0u32;
        let mut i = count;
        while i > 0 && found < limit {
            i -= 1;
            if let Some(record) = env
                .storage()
                .instance()
                .get::<DataKey, EventRecord>(&DataKey::Event(i))
            {
                results.push_back(record);
                found += 1;
            }
        }
        results
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{EventRegistry, EventRegistryClient, RegistryError};
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

    fn setup() -> (Env, EventRegistryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(EventRegistry, ());
        let client = EventRegistryClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_register_and_get_event() {
        let (env, client, _admin) = setup();
        let contract = Address::generate(&env);
        let actor = Address::generate(&env);

        let event_id = client.register_event(
            &symbol_short!("amm"),
            &symbol_short!("swap"),
            &contract,
            &actor,
            &symbol_short!("data"),
        );

        assert_eq!(event_id, 0);
        assert_eq!(client.event_count(), 1);

        let record = client.get_event(&event_id);
        assert_eq!(record.primitive, symbol_short!("amm"));
        assert_eq!(record.event_type, symbol_short!("swap"));
    }

    #[test]
    fn test_list_by_primitive() {
        let (env, client, _admin) = setup();
        let contract = Address::generate(&env);
        let actor = Address::generate(&env);

        client.register_event(&symbol_short!("amm"), &symbol_short!("swap"), &contract, &actor, &symbol_short!("d1"));
        client.register_event(&symbol_short!("lending"), &symbol_short!("borrow"), &contract, &actor, &symbol_short!("d2"));
        client.register_event(&symbol_short!("amm"), &symbol_short!("add_liq"), &contract, &actor, &symbol_short!("d3"));

        let amm_events = client.list_by_primitive(&symbol_short!("amm"), &10);
        assert_eq!(amm_events.len(), 2);

        let lending_events = client.list_by_primitive(&symbol_short!("lending"), &10);
        assert_eq!(lending_events.len(), 1);
    }

    #[test]
    fn test_list_by_actor() {
        let (env, client, _admin) = setup();
        let contract = Address::generate(&env);
        let actor1 = Address::generate(&env);
        let actor2 = Address::generate(&env);

        client.register_event(&symbol_short!("amm"), &symbol_short!("swap"), &contract, &actor1, &symbol_short!("d1"));
        client.register_event(&symbol_short!("amm"), &symbol_short!("swap"), &contract, &actor2, &symbol_short!("d2"));
        client.register_event(&symbol_short!("escrow"), &symbol_short!("fund"), &contract, &actor1, &symbol_short!("d3"));

        let actor1_events = client.list_by_actor(&actor1, &10);
        assert_eq!(actor1_events.len(), 2);
    }

    #[test]
    fn test_get_nonexistent_event() {
        let (env, client, _admin) = setup();
        let result = client.try_get_event(&999);
        assert_eq!(result, Err(Ok(RegistryError::EventNotFound)));
    }

    #[test]
    fn test_list_recent() {
        let (env, client, _admin) = setup();
        let contract = Address::generate(&env);
        let actor = Address::generate(&env);

        for _ in 0..5 {
            client.register_event(&symbol_short!("amm"), &symbol_short!("swap"), &contract, &actor, &symbol_short!("d"));
        }

        let recent = client.list_recent(&3);
        assert_eq!(recent.len(), 3);
    }
}
