#![no_std]

//! Escrow Factory — deploys and catalogs escrow contract instances.
//! Third-party apps can deploy their own escrow instances via this factory.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Symbol,
    Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscrowInstance {
    pub instance_id: u32,
    pub contract_address: Address,
    pub creator: Address,
    pub name: Symbol,
    pub active: bool,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    InstanceCount,
    Instance(u32),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FactoryError {
    NotAdmin = 1,
    AlreadyInitialized = 2,
    InstanceNotFound = 3,
    NotCreator = 4,
}

#[contract]
pub struct EscrowFactory;

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

fn next_instance_id(env: &Env) -> u32 {
    let count: u32 = env
        .storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::InstanceCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::InstanceCount, &(count + 1));
    count
}

#[contractimpl]
impl EscrowFactory {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::InstanceCount, &0u32);
    }

    /// Deploy a new escrow instance from a pre-uploaded WASM hash and return its
    /// address. The caller is responsible for initializing the deployed instance.
    /// NOTE: the deploy→initialize gap is a known TOCTOU window (see remediation
    /// plan 2.7); fully closing it requires a shared client-crate refactor and is deferred.
    pub fn create_escrow(
        env: Env,
        caller: Address,
        wasm_hash: BytesN<32>,
        salt: BytesN<32>,
        name: Symbol,
    ) -> Result<(u32, Address), FactoryError> {
        caller.require_auth();

        let contract_address = env
            .deployer()
            .with_current_contract(salt)
            .deploy(wasm_hash);

        let instance_id = next_instance_id(&env);
        let instance = EscrowInstance {
            instance_id,
            contract_address: contract_address.clone(),
            creator: caller,
            name,
            active: true,
            created_at: env.ledger().timestamp(),
        };

        env.storage()
            .instance()
            .set(&DataKey::Instance(instance_id), &instance);

        env.events().publish(
            (symbol_short!("esc_new"), instance_id),
            &contract_address,
        );

        Ok((instance_id, contract_address))
    }

    /// Register an existing escrow contract. Admin only.
    pub fn register_escrow(
        env: Env,
        admin: Address,
        contract_address: Address,
        name: Symbol,
    ) -> Result<u32, FactoryError> {
        if admin != read_admin(&env) {
            return Err(FactoryError::NotAdmin);
        }
        admin.require_auth();

        let instance_id = next_instance_id(&env);
        let instance = EscrowInstance {
            instance_id,
            contract_address: contract_address.clone(),
            creator: admin,
            name,
            active: true,
            created_at: env.ledger().timestamp(),
        };

        env.storage()
            .instance()
            .set(&DataKey::Instance(instance_id), &instance);

        env.events().publish(
            (symbol_short!("esc_reg"), instance_id),
            &contract_address,
        );

        Ok(instance_id)
    }

    /// Deactivate an escrow instance. Creator or admin.
    pub fn remove_escrow(
        env: Env,
        caller: Address,
        instance_id: u32,
    ) -> Result<(), FactoryError> {
        caller.require_auth();

        let mut instance: EscrowInstance = env
            .storage()
            .instance()
            .get::<DataKey, EscrowInstance>(&DataKey::Instance(instance_id))
            .ok_or(FactoryError::InstanceNotFound)?;

        let admin = read_admin(&env);
        if caller != instance.creator && caller != admin {
            return Err(FactoryError::NotCreator);
        }

        instance.active = false;
        env.storage()
            .instance()
            .set(&DataKey::Instance(instance_id), &instance);

        env.events()
            .publish((symbol_short!("esc_rm"), instance_id), &instance.contract_address);

        Ok(())
    }

    /// Get instance info.
    pub fn get_escrow(env: Env, instance_id: u32) -> Result<EscrowInstance, FactoryError> {
        env.storage()
            .instance()
            .get::<DataKey, EscrowInstance>(&DataKey::Instance(instance_id))
            .ok_or(FactoryError::InstanceNotFound)
    }

    /// List all active escrow instances.
    pub fn list_escrows(env: Env) -> Vec<EscrowInstance> {
        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::InstanceCount)
            .unwrap_or(0);
        let mut results = Vec::new(&env);
        for i in 0..count {
            if let Some(instance) = env
                .storage()
                .instance()
                .get::<DataKey, EscrowInstance>(&DataKey::Instance(i))
            {
                if instance.active {
                    results.push_back(instance);
                }
            }
        }
        results
    }

    /// List all instances including inactive.
    pub fn list_all_escrows(env: Env) -> Vec<EscrowInstance> {
        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::InstanceCount)
            .unwrap_or(0);
        let mut results = Vec::new(&env);
        for i in 0..count {
            if let Some(instance) = env
                .storage()
                .instance()
                .get::<DataKey, EscrowInstance>(&DataKey::Instance(i))
            {
                results.push_back(instance);
            }
        }
        results
    }

    pub fn instance_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::InstanceCount)
            .unwrap_or(0)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{EscrowFactory, EscrowFactoryClient, FactoryError};
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, BytesN, Env};

    fn setup() -> (Env, EscrowFactoryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(EscrowFactory, ());
        let client = EscrowFactoryClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_register_escrow() {
        let (env, client, admin) = setup();
        let contract = Address::generate(&env);

        let instance_id = client.register_escrow(&admin, &contract, &symbol_short!("test_esc"));
        assert_eq!(instance_id, 0);
        assert_eq!(client.instance_count(), 1);

        let instance = client.get_escrow(&instance_id);
        assert_eq!(instance.contract_address, contract);
        assert!(instance.active);
    }

    #[test]
    fn test_list_escrows() {
        let (env, client, admin) = setup();
        let c1 = Address::generate(&env);
        let c2 = Address::generate(&env);

        client.register_escrow(&admin, &c1, &symbol_short!("esc1"));
        client.register_escrow(&admin, &c2, &symbol_short!("esc2"));

        let list = client.list_escrows();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_remove_escrow() {
        let (env, client, admin) = setup();
        let contract = Address::generate(&env);

        let id = client.register_escrow(&admin, &contract, &symbol_short!("esc"));
        client.remove_escrow(&admin, &id);

        let instance = client.get_escrow(&id);
        assert!(!instance.active);

        let list = client.list_escrows();
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_non_creator_cannot_remove() {
        let (env, client, admin) = setup();
        let contract = Address::generate(&env);

        let id = client.register_escrow(&admin, &contract, &symbol_short!("esc"));
        let rando = Address::generate(&env);

        let result = client.try_remove_escrow(&rando, &id);
        assert_eq!(result, Err(Ok(FactoryError::NotCreator)));
    }

    #[test]
    fn test_not_found() {
        let (env, client, _admin) = setup();
        let result = client.try_get_escrow(&999);
        assert_eq!(result, Err(Ok(FactoryError::InstanceNotFound)));
    }
}
