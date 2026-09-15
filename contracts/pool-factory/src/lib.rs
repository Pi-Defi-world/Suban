#![no_std]

//! Pool Factory — deploys and catalogs AMM pools.
//! Third-party developers can deploy their own isolated pools
//! by calling `create_pool()` with a pre-uploaded WASM hash.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Symbol,
    Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PoolType {
    ConstantProduct = 0,
    Stableswap = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolInfo {
    pub pool_id: u32,
    pub pool_address: Address,
    pub token_a: Address,
    pub token_b: Address,
    pub pool_type: PoolType,
    pub fee_bps: u32,
    pub creator: Address,
    pub name: Symbol,
    pub active: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    PoolCount,
    Pool(u32),
    PairIndex(Address, Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FactoryError {
    NotAdmin = 1,
    AlreadyInitialized = 2,
    PoolNotFound = 3,
    PairAlreadyExists = 4,
    InvalidFee = 5,
    NotCreator = 6,
}

#[contract]
pub struct PoolFactory;

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

fn next_pool_id(env: &Env) -> u32 {
    let count: u32 = env
        .storage()
        .instance()
        .get::<DataKey, u32>(&DataKey::PoolCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::PoolCount, &(count + 1));
    count
}

fn pair_key(a: &Address, b: &Address) -> (Address, Address) {
    if a < b {
        (a.clone(), b.clone())
    } else {
        (b.clone(), a.clone())
    }
}

#[contractimpl]
impl PoolFactory {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::PoolCount, &0u32);
    }

    /// Deploy a new pool from a pre-uploaded WASM hash.
    /// Caller must upload WASM beforehand (`env.deployer().upload_contract_wasm`)
    /// and then initialize the deployed pool itself. NOTE: the deploy→initialize
    /// gap is a known TOCTOU window (see remediation plan 2.7); fully closing it
    /// requires a shared client-crate refactor and is deferred.
    pub fn create_pool(
        env: Env,
        caller: Address,
        wasm_hash: BytesN<32>,
        salt: BytesN<32>,
        token_a: Address,
        token_b: Address,
        pool_type: u32,
        fee_bps: u32,
        name: Symbol,
    ) -> Result<(u32, Address), FactoryError> {
        caller.require_auth();
        if fee_bps > 1000 {
            return Err(FactoryError::InvalidFee);
        }

        let ptype = match pool_type {
            0 => PoolType::ConstantProduct,
            1 => PoolType::Stableswap,
            _ => return Err(FactoryError::InvalidFee),
        };

        let (ka, kb) = pair_key(&token_a, &token_b);
        let idx_key = DataKey::PairIndex(ka, kb);
        if env.storage().instance().has(&idx_key) {
            return Err(FactoryError::PairAlreadyExists);
        }

        let pool_address = env
            .deployer()
            .with_current_contract(salt)
            .deploy(wasm_hash);

        let pool_id = next_pool_id(&env);
        let info = PoolInfo {
            pool_id,
            pool_address: pool_address.clone(),
            token_a: token_a.clone(),
            token_b: token_b.clone(),
            pool_type: ptype,
            fee_bps,
            creator: caller,
            name,
            active: true,
        };

        env.storage().instance().set(&DataKey::Pool(pool_id), &info);
        env.storage().instance().set(&idx_key, &pool_id);

        env.events().publish(
            (symbol_short!("pool_new"), pool_id),
            (&token_a, &token_b),
        );

        Ok((pool_id, pool_address))
    }

    /// Register an existing (pre-deployed) pool. Admin only.
    pub fn register_pool(
        env: Env,
        admin: Address,
        pool_address: Address,
        token_a: Address,
        token_b: Address,
        pool_type: u32,
        fee_bps: u32,
    ) -> Result<u32, FactoryError> {
        if admin != read_admin(&env) {
            return Err(FactoryError::NotAdmin);
        }
        admin.require_auth();
        if fee_bps > 1000 {
            return Err(FactoryError::InvalidFee);
        }

        let ptype = match pool_type {
            0 => PoolType::ConstantProduct,
            1 => PoolType::Stableswap,
            _ => return Err(FactoryError::InvalidFee),
        };

        let (ka, kb) = pair_key(&token_a, &token_b);
        let idx_key = DataKey::PairIndex(ka, kb);
        if env.storage().instance().has(&idx_key) {
            return Err(FactoryError::PairAlreadyExists);
        }

        let pool_id = next_pool_id(&env);
        let info = PoolInfo {
            pool_id,
            pool_address,
            token_a: token_a.clone(),
            token_b: token_b.clone(),
            pool_type: ptype,
            fee_bps,
            creator: admin.clone(),
            name: symbol_short!("legacy"),
            active: true,
        };

        env.storage().instance().set(&DataKey::Pool(pool_id), &info);
        env.storage().instance().set(&idx_key, &pool_id);

        env.events().publish(
            (symbol_short!("pool_reg"), pool_id),
            (&token_a, &token_b),
        );

        Ok(pool_id)
    }

    /// Deactivate a pool. Creator or admin can call.
    pub fn remove_pool(
        env: Env,
        caller: Address,
        pool_id: u32,
    ) -> Result<(), FactoryError> {
        caller.require_auth();
        let mut info: PoolInfo = env
            .storage()
            .instance()
            .get::<DataKey, PoolInfo>(&DataKey::Pool(pool_id))
            .ok_or(FactoryError::PoolNotFound)?;

        let admin = read_admin(&env);
        if caller != info.creator && caller != admin {
            return Err(FactoryError::NotCreator);
        }

        info.active = false;
        env.storage().instance().set(&DataKey::Pool(pool_id), &info);

        env.events()
            .publish((symbol_short!("pool_rm"), pool_id), &info.pool_address);

        Ok(())
    }

    /// Reactivate a pool. Admin only.
    pub fn reactivate_pool(
        env: Env,
        admin: Address,
        pool_id: u32,
    ) -> Result<(), FactoryError> {
        if admin != read_admin(&env) {
            return Err(FactoryError::NotAdmin);
        }
        admin.require_auth();
        let mut info: PoolInfo = env
            .storage()
            .instance()
            .get::<DataKey, PoolInfo>(&DataKey::Pool(pool_id))
            .ok_or(FactoryError::PoolNotFound)?;

        info.active = true;
        env.storage().instance().set(&DataKey::Pool(pool_id), &info);

        Ok(())
    }

    /// Get pool info by ID.
    pub fn get_pool(env: Env, pool_id: u32) -> Result<PoolInfo, FactoryError> {
        env.storage()
            .instance()
            .get::<DataKey, PoolInfo>(&DataKey::Pool(pool_id))
            .ok_or(FactoryError::PoolNotFound)
    }

    /// Find pool by token pair (order-independent).
    pub fn find_pool(
        env: Env,
        token_a: Address,
        token_b: Address,
    ) -> Result<PoolInfo, FactoryError> {
        let (ka, kb) = pair_key(&token_a, &token_b);
        let pool_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PairIndex(ka, kb))
            .ok_or(FactoryError::PoolNotFound)?;
        env.storage()
            .instance()
            .get(&DataKey::Pool(pool_id))
            .ok_or(FactoryError::PoolNotFound)
    }

    /// List all active pools.
    pub fn list_pools(env: Env) -> Vec<PoolInfo> {
        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::PoolCount)
            .unwrap_or(0);
        let mut pools = Vec::new(&env);
        for i in 0..count {
            if let Some(info) = env
                .storage()
                .instance()
                .get::<DataKey, PoolInfo>(&DataKey::Pool(i))
            {
                if info.active {
                    pools.push_back(info);
                }
            }
        }
        pools
    }

    /// List all pools including inactive.
    pub fn list_all_pools(env: Env) -> Vec<PoolInfo> {
        let count: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::PoolCount)
            .unwrap_or(0);
        let mut pools = Vec::new(&env);
        for i in 0..count {
            if let Some(info) = env
                .storage()
                .instance()
                .get::<DataKey, PoolInfo>(&DataKey::Pool(i))
            {
                pools.push_back(info);
            }
        }
        pools
    }

    pub fn pool_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::PoolCount)
            .unwrap_or(0)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{FactoryError, PoolFactory, PoolFactoryClient, PoolType};
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

    fn setup() -> (Env, PoolFactoryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(PoolFactory, ());
        let client = PoolFactoryClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_register_pool() {
        let (env, client, admin) = setup();
        let pool_addr = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        let pool_id = client.register_pool(
            &admin, &pool_addr, &token_a, &token_b, &0u32, &30,
        );
        assert_eq!(pool_id, 0);
        assert_eq!(client.pool_count(), 1);

        let info = client.get_pool(&pool_id);
        assert_eq!(info.pool_address, pool_addr);
        assert_eq!(info.pool_type, PoolType::ConstantProduct);
        assert!(info.active);
    }

    #[test]
    fn test_find_pool() {
        let (env, client, admin) = setup();
        let pool_addr = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        client.register_pool(
            &admin, &pool_addr, &token_a, &token_b, &0u32, &30,
        );

        let found = client.find_pool(&token_a, &token_b);
        assert_eq!(found.pool_address, pool_addr);

        let found2 = client.find_pool(&token_b, &token_a);
        assert_eq!(found2.pool_address, pool_addr);
    }

    #[test]
    fn test_duplicate_pair_fails() {
        let (env, client, admin) = setup();
        let pool_addr = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        client.register_pool(
            &admin, &pool_addr, &token_a, &token_b, &0u32, &30,
        );

        let pool_addr2 = Address::generate(&env);
        let result = client.try_register_pool(
            &admin, &pool_addr2, &token_a, &token_b, &1u32, &10,
        );
        assert_eq!(result, Err(Ok(FactoryError::PairAlreadyExists)));
    }

    #[test]
    fn test_non_admin_fails() {
        let (env, client, _) = setup();
        let rando = Address::generate(&env);
        let pool_addr = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        let result = client.try_register_pool(
            &rando, &pool_addr, &token_a, &token_b, &0u32, &30,
        );
        assert_eq!(result, Err(Ok(FactoryError::NotAdmin)));
    }

    #[test]
    fn test_list_pools() {
        let (env, client, admin) = setup();
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);
        let token_c = Address::generate(&env);

        client.register_pool(
            &admin, &Address::generate(&env), &token_a, &token_b, &0u32, &30,
        );
        client.register_pool(
            &admin, &Address::generate(&env), &token_b, &token_c, &1u32, &10,
        );

        let pools = client.list_pools();
        assert_eq!(pools.len(), 2);
    }

    #[test]
    fn test_remove_pool() {
        let (env, client, admin) = setup();
        let pool_addr = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        let pool_id = client.register_pool(
            &admin, &pool_addr, &token_a, &token_b, &0u32, &30,
        );

        client.remove_pool(&admin, &pool_id);
        let info = client.get_pool(&pool_id);
        assert!(!info.active);

        let pools = client.list_pools();
        assert_eq!(pools.len(), 0);

        let all = client.list_all_pools();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn test_creator_can_remove() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        // Register via admin (creator = admin in this case)
        let pool_addr = Address::generate(&env);
        let pool_id = client.register_pool(
            &admin, &pool_addr, &token_a, &token_b, &0u32, &30,
        );

        // Admin (who is creator) can remove
        client.remove_pool(&admin, &pool_id);
        let info = client.get_pool(&pool_id);
        assert!(!info.active);
    }

    #[test]
    fn test_non_creator_non_admin_cannot_remove() {
        let (env, client, admin) = setup();
        let pool_addr = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        let pool_id = client.register_pool(
            &admin, &pool_addr, &token_a, &token_b, &0u32, &30,
        );

        let rando = Address::generate(&env);
        let result = client.try_remove_pool(&rando, &pool_id);
        assert_eq!(result, Err(Ok(FactoryError::NotCreator)));
    }

    #[test]
    fn test_reactivate_pool() {
        let (env, client, admin) = setup();
        let pool_addr = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        let pool_id = client.register_pool(
            &admin, &pool_addr, &token_a, &token_b, &0u32, &30,
        );

        client.remove_pool(&admin, &pool_id);
        assert!(!client.get_pool(&pool_id).active);

        client.reactivate_pool(&admin, &pool_id);
        assert!(client.get_pool(&pool_id).active);
    }

    #[test]
    fn test_invalid_fee() {
        let (env, client, admin) = setup();
        let pool_addr = Address::generate(&env);
        let token_a = Address::generate(&env);
        let token_b = Address::generate(&env);

        let result = client.try_register_pool(
            &admin, &pool_addr, &token_a, &token_b, &0u32, &1001,
        );
        assert_eq!(result, Err(Ok(FactoryError::InvalidFee)));
    }
}
