#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol};
use hub_types::FeeConfig;

#[contract]
pub struct FeeRouter;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    FeeConfig(Symbol),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FeeError {
    NotAdmin = 1,
    InvalidBps = 2,
    NoRoutes = 3,
    TotalBpsMismatch = 5,
}

fn read_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Admin)
        .unwrap()
}

#[contractimpl]
impl FeeRouter {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Set fee configuration for a fee type (e.g., "swap", "lending", "escrow").
    /// Total shares must equal 10000 (100%).
    pub fn set_fee_config(
        env: Env,
        admin: Address,
        fee_type: Symbol,
        config: FeeConfig,
    ) -> Result<(), FeeError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(FeeError::NotAdmin);
        }
        admin.require_auth();

        let total = config.lp_share + config.protocol_share + config.backstop_share + config.platform_share;
        if total != 10000 {
            return Err(FeeError::TotalBpsMismatch);
        }

        env.storage()
            .instance()
            .set(&DataKey::FeeConfig(fee_type.clone()), &config);

        env.events().publish(
            (symbol_short!("fee_cfg"), fee_type),
            (config.lp_share, config.protocol_share, config.backstop_share, config.platform_share),
        );

        Ok(())
    }

    /// Get fee configuration for a fee type.
    pub fn get_fee_config(env: Env, fee_type: Symbol) -> Option<FeeConfig> {
        env.storage()
            .instance()
            .get::<DataKey, FeeConfig>(&DataKey::FeeConfig(fee_type))
    }

    /// Route fees from a source address according to the fee config.
    /// Splits the total_fee among LP, protocol, backstop, and platform destinations.
    pub fn route_fees(
        env: Env,
        source: Address,
        fee_type: Symbol,
        total_fee: i128,
        lp_dest: Address,
        protocol_dest: Address,
        backstop_dest: Address,
        platform_dest: Address,
    ) -> Result<(), FeeError> {
        let config = env
            .storage()
            .instance()
            .get::<DataKey, FeeConfig>(&DataKey::FeeConfig(fee_type.clone()))
            .ok_or(FeeError::NoRoutes)?;

        let lp_amount = total_fee * config.lp_share as i128 / 10000;
        let protocol_amount = total_fee * config.protocol_share as i128 / 10000;
        let backstop_amount = total_fee * config.backstop_share as i128 / 10000;
        let platform_amount = total_fee - lp_amount - protocol_amount - backstop_amount;

        if lp_amount > 0 {
            env.events().publish(
                (symbol_short!("fee_rt"), fee_type.clone()),
                (source.clone(), lp_dest, lp_amount),
            );
        }

        if protocol_amount > 0 {
            env.events().publish(
                (symbol_short!("fee_rt"), fee_type.clone()),
                (source.clone(), protocol_dest, protocol_amount),
            );
        }

        if backstop_amount > 0 {
            env.events().publish(
                (symbol_short!("fee_rt"), fee_type.clone()),
                (source.clone(), backstop_dest, backstop_amount),
            );
        }

        if platform_amount > 0 {
            env.events().publish(
                (symbol_short!("fee_rt"), fee_type),
                (source, platform_dest, platform_amount),
            );
        }

        Ok(())
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env)
    }

    pub fn set_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), FeeError> {
        let current_admin = read_admin(&env);
        if admin != current_admin {
            return Err(FeeError::NotAdmin);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::{FeeError, FeeRouter, FeeRouterClient};
    use hub_types::FeeConfig;
    use soroban_sdk::{testutils::Address as _, Address, Env, symbol_short};

    fn setup() -> (Env, FeeRouterClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(FeeRouter, ());
        let client = FeeRouterClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn test_set_fee_config() {
        let (_env, client, admin) = setup();

        let config = FeeConfig {
            lp_share: 5000,
            protocol_share: 2000,
            backstop_share: 2000,
            platform_share: 1000,
        };

        client.set_fee_config(&admin, &symbol_short!("swap"), &config);

        let retrieved = client.get_fee_config(&symbol_short!("swap"));
        assert_eq!(retrieved, Some(config));
    }

    #[test]
    fn test_invalid_total_bps() {
        let (_env, client, admin) = setup();

        let config = FeeConfig {
            lp_share: 5000,
            protocol_share: 2000,
            backstop_share: 2000,
            platform_share: 500,
        };

        let result = client.try_set_fee_config(&admin, &symbol_short!("swap"), &config);
        assert_eq!(result, Err(Ok(FeeError::TotalBpsMismatch)));
    }
}
