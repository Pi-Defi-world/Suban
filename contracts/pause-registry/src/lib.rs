#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol};
use hub_types::Primitive;

#[contract]
pub struct PauseRegistry;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Paused(Primitive),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PauseError {
    NotAdmin = 1,
    AlreadyPaused = 2,
    NotPaused = 3,
    CannotPauseAll = 4,
}

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

fn is_paused(env: &Env, primitive: &Primitive) -> bool {
    env.storage()
        .instance()
        .get::<DataKey, bool>(&DataKey::Paused(primitive.clone()))
        .unwrap_or(false)
}

fn set_paused(env: &Env, primitive: &Primitive, paused: bool) {
    env.storage()
        .instance()
        .set(&DataKey::Paused(primitive.clone()), &paused);
}

fn primitive_topic(primitive: &Primitive) -> Symbol {
    match primitive {
        Primitive::Amm => symbol_short!("amm"),
        Primitive::Lending => symbol_short!("lend"),
        Primitive::Escrow => symbol_short!("escrow"),
        Primitive::Bridge => symbol_short!("bridge"),
        Primitive::All => symbol_short!("all"),
    }
}

#[contractimpl]
impl PauseRegistry {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn pause(env: Env, admin: Address, primitive: Primitive) -> Result<(), PauseError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(PauseError::NotAdmin);
        }
        admin.require_auth();

        if is_paused(&env, &primitive) {
            return Err(PauseError::AlreadyPaused);
        }

        set_paused(&env, &primitive, true);

        env.events().publish(
            (symbol_short!("pause"), primitive_topic(&primitive)),
            admin,
        );

        Ok(())
    }

    pub fn unpause(env: Env, admin: Address, primitive: Primitive) -> Result<(), PauseError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(PauseError::NotAdmin);
        }
        admin.require_auth();

        if !is_paused(&env, &primitive) {
            return Err(PauseError::NotPaused);
        }

        set_paused(&env, &primitive, false);

        env.events().publish(
            (symbol_short!("unpause"), primitive_topic(&primitive)),
            admin,
        );

        Ok(())
    }

    pub fn is_paused(env: Env, primitive: Primitive) -> bool {
        is_paused(&env, &primitive)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }

    pub fn set_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), PauseError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(PauseError::NotAdmin);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{PauseError, PauseRegistry, PauseRegistryClient};
    use hub_types::Primitive;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, PauseRegistryClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PauseRegistry, ());
        let client = PauseRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_pause_unpause() {
        let (_env, client, admin) = setup();

        assert!(!client.is_paused(&Primitive::Amm));

        client.pause(&admin, &Primitive::Amm);
        assert!(client.is_paused(&Primitive::Amm));

        client.unpause(&admin, &Primitive::Amm);
        assert!(!client.is_paused(&Primitive::Amm));
    }

    #[test]
    fn test_double_pause_fails() {
        let (_env, client, admin) = setup();

        client.pause(&admin, &Primitive::Lending);
        let result = client.try_pause(&admin, &Primitive::Lending);
        assert_eq!(result, Err(Ok(PauseError::AlreadyPaused)));
    }

    #[test]
    fn test_non_admin_cannot_pause() {
        let (test_env, client, _admin) = setup();
        let rando = Address::generate(&test_env);

        let result = client.try_pause(&rando, &Primitive::Escrow);
        assert_eq!(result, Err(Ok(PauseError::NotAdmin)));
    }
}
