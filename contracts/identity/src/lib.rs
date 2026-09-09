#![no_std]

//! Identity Resolution — maps Pi accounts to on-chain Soroban addresses.
//! Enables escrow, lending, and other primitives to resolve identities
//! across Pi Network and Stellar/Soroban.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Identity {
    pub pi_account: Symbol,      // Pi Network username or ID
    pub soroban_address: Address, // Soroban public key
    pub verified: bool,
    pub registered_at: u64,
    pub metadata: Symbol,        // optional metadata (JSON string)
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    IdentityByPi(Symbol),
    IdentityBySoroban(Address),
    AllIdentities(u32),
    IdentityCount,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum IdentityError {
    NotAdmin = 1,
    AlreadyInitialized = 2,
    IdentityNotFound = 3,
    AlreadyRegistered = 4,
    NotVerified = 5,
}

#[contract]
pub struct IdentityResolver;

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

#[contractimpl]
impl IdentityResolver {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::IdentityCount, &0u32);
    }

    /// Register a new identity mapping. Admin only.
    pub fn register_identity(
        env: Env,
        pi_account: Symbol,
        soroban_address: Address,
        metadata: Symbol,
    ) -> Result<(), IdentityError> {
        let admin = read_admin(&env);
        admin.require_auth();

        if env.storage().instance().has(&DataKey::IdentityByPi(pi_account.clone())) {
            return Err(IdentityError::AlreadyRegistered);
        }

        if env.storage().instance().has(&DataKey::IdentityBySoroban(soroban_address.clone())) {
            return Err(IdentityError::AlreadyRegistered);
        }

        let identity = Identity {
            pi_account: pi_account.clone(),
            soroban_address: soroban_address.clone(),
            verified: true,
            registered_at: env.ledger().timestamp(),
            metadata,
        };

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::IdentityCount)
            .unwrap_or(0);

        env.storage().instance().set(&DataKey::IdentityByPi(pi_account), &identity);
        env.storage().instance().set(&DataKey::IdentityBySoroban(soroban_address), &identity);
        env.storage().instance().set(&DataKey::AllIdentities(count), &identity);
        env.storage().instance().set(&DataKey::IdentityCount, &(count + 1));

        env.events().publish(
            (symbol_short!("id_reg"), &identity.soroban_address),
            &identity.pi_account,
        );

        Ok(())
    }

    /// Verify an identity. Admin only.
    pub fn verify_identity(
        env: Env,
        pi_account: Symbol,
    ) -> Result<(), IdentityError> {
        let admin = read_admin(&env);
        admin.require_auth();

        let mut identity: Identity = env
            .storage()
            .instance()
            .get(&DataKey::IdentityByPi(pi_account))
            .ok_or(IdentityError::IdentityNotFound)?;

        identity.verified = true;
        env.storage().instance().set(&DataKey::IdentityByPi(identity.pi_account.clone()), &identity);
        env.storage().instance().set(&DataKey::IdentityBySoroban(identity.soroban_address.clone()), &identity);

        Ok(())
    }

    /// Lookup identity by Pi account.
    pub fn resolve_by_pi(env: Env, pi_account: Symbol) -> Result<Identity, IdentityError> {
        env.storage()
            .instance()
            .get::<DataKey, Identity>(&DataKey::IdentityByPi(pi_account))
            .ok_or(IdentityError::IdentityNotFound)
    }

    /// Lookup identity by Soroban address.
    pub fn resolve_by_soroban(env: Env, soroban_address: Address) -> Result<Identity, IdentityError> {
        env.storage()
            .instance()
            .get::<DataKey, Identity>(&DataKey::IdentityBySoroban(soroban_address))
            .ok_or(IdentityError::IdentityNotFound)
    }

    /// Check if an identity is verified.
    pub fn is_verified(env: Env, pi_account: Symbol) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, Identity>(&DataKey::IdentityByPi(pi_account))
            .map(|i| i.verified)
            .unwrap_or(false)
    }

    /// Get total identity count.
    pub fn identity_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::IdentityCount)
            .unwrap_or(0)
    }

    /// List all identities (up to limit).
    pub fn list_identities(env: Env, limit: u32) -> Vec<Identity> {
        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::IdentityCount)
            .unwrap_or(0);

        let mut results = Vec::new(&env);
        let mut i = 0u32;
        while i < count && i < limit {
            if let Some(identity) = env
                .storage()
                .instance()
                .get::<DataKey, Identity>(&DataKey::AllIdentities(i))
            {
                results.push_back(identity);
            }
            i += 1;
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
    use super::{IdentityError, IdentityResolver, IdentityResolverClient};
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

    fn setup() -> (Env, IdentityResolverClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(IdentityResolver, ());
        let client = IdentityResolverClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_register_identity() {
        let (env, client, _admin) = setup();
        let soroban = Address::generate(&env);

        client.register_identity(&symbol_short!("alice"), &soroban, &symbol_short!("meta"));

        assert_eq!(client.identity_count(), 1);
        assert!(client.is_verified(&symbol_short!("alice")));
    }

    #[test]
    fn test_resolve_by_pi() {
        let (env, client, _admin) = setup();
        let soroban = Address::generate(&env);

        client.register_identity(&symbol_short!("bob"), &soroban, &symbol_short!("meta"));

        let identity = client.resolve_by_pi(&symbol_short!("bob"));
        assert_eq!(identity.soroban_address, soroban);
        assert!(identity.verified);
    }

    #[test]
    fn test_resolve_by_soroban() {
        let (env, client, _admin) = setup();
        let soroban = Address::generate(&env);

        client.register_identity(&symbol_short!("carol"), &soroban, &symbol_short!("meta"));

        let identity = client.resolve_by_soroban(&soroban);
        assert_eq!(identity.pi_account, symbol_short!("carol"));
    }

    #[test]
    fn test_duplicate_fails() {
        let (env, client, _admin) = setup();
        let soroban = Address::generate(&env);

        client.register_identity(&symbol_short!("alice"), &soroban, &symbol_short!("meta"));

        let soroban2 = Address::generate(&env);
        let result = client.try_register_identity(&symbol_short!("alice"), &soroban2, &symbol_short!("m2"));
        assert_eq!(result, Err(Ok(IdentityError::AlreadyRegistered)));
    }

    #[test]
    fn test_not_found() {
        let (env, client, _admin) = setup();
        let result = client.try_resolve_by_pi(&symbol_short!("nobody"));
        assert_eq!(result, Err(Ok(IdentityError::IdentityNotFound)));
    }
}
