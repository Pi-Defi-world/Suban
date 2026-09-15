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
        token: Address,
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

        let asset = soroban_sdk::token::Client::new(&env, &token);
        let mut routed = 0i128;

        if lp_amount > 0 {
            asset.transfer(&source, &lp_dest, &lp_amount);
            routed += lp_amount;
            env.events().publish(
                (symbol_short!("fee_rt"), fee_type.clone()),
                (source.clone(), lp_dest, lp_amount),
            );
        }

        if protocol_amount > 0 {
            asset.transfer(&source, &protocol_dest, &protocol_amount);
            routed += protocol_amount;
            env.events().publish(
                (symbol_short!("fee_rt"), fee_type.clone()),
                (source.clone(), protocol_dest, protocol_amount),
            );
        }

        if backstop_amount > 0 {
            asset.transfer(&source, &backstop_dest, &backstop_amount);
            routed += backstop_amount;
            env.events().publish(
                (symbol_short!("fee_rt"), fee_type.clone()),
                (source.clone(), backstop_dest, backstop_amount),
            );
        }

        if platform_amount > 0 {
            asset.transfer(&source, &platform_dest, &platform_amount);
            routed += platform_amount;
            env.events().publish(
                (symbol_short!("fee_rt"), fee_type),
                (source.clone(), platform_dest, platform_amount),
            );
        }

        // The full fee must have been routed out of the source.
        assert_eq!(routed, total_fee);

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
    use soroban_sdk::{
        contract, contractimpl, contracttype, testutils::Address as _, Address, Env, symbol_short,
    };

    #[contract]
    struct MockToken;

    #[contracttype]
    enum MockKey {
        Bal(Address),
    }

    #[contractimpl]
    impl MockToken {
        pub fn initialize(_env: Env, _admin: Address) {}
        pub fn mint(env: Env, to: Address, amount: i128) {
            let b: i128 = env
                .storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(to.clone()))
                .unwrap_or(0);
            env.storage().instance().set(&MockKey::Bal(to), &(b + amount));
        }
        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            let fb: i128 = env
                .storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(from.clone()))
                .unwrap_or(0);
            let tb: i128 = env
                .storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(to.clone()))
                .unwrap_or(0);
            env.storage().instance().set(&MockKey::Bal(from), &(fb - amount));
            env.storage().instance().set(&MockKey::Bal(to), &(tb + amount));
        }
        pub fn balance(env: Env, addr: Address) -> i128 {
            env.storage()
                .instance()
                .get::<MockKey, i128>(&MockKey::Bal(addr))
                .unwrap_or(0)
        }
    }

    fn setup() -> (Env, FeeRouterClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(FeeRouter, ());
        let client = FeeRouterClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let token = env.register(MockToken, ());
        MockTokenClient::new(&env, &token).initialize(&admin);
        (env, client, admin, token)
    }

    #[test]
    fn test_set_fee_config() {
        let (_env, client, admin, _token) = setup();

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
        let (_env, client, admin, _token) = setup();

        let config = FeeConfig {
            lp_share: 5000,
            protocol_share: 2000,
            backstop_share: 2000,
            platform_share: 500,
        };

        let result = client.try_set_fee_config(&admin, &symbol_short!("swap"), &config);
        assert_eq!(result, Err(Ok(FeeError::TotalBpsMismatch)));
    }

    #[test]
    fn test_route_fees_custody() {
        let (env, client, admin, token) = setup();

        let config = FeeConfig {
            lp_share: 5000,
            protocol_share: 2000,
            backstop_share: 2000,
            platform_share: 1000,
        };
        client.set_fee_config(&admin, &symbol_short!("swap"), &config);

        let source = Address::generate(&env);
        let lp_dest = Address::generate(&env);
        let protocol_dest = Address::generate(&env);
        let backstop_dest = Address::generate(&env);
        let platform_dest = Address::generate(&env);

        MockTokenClient::new(&env, &token).mint(&source, &1_000_000);

        client.route_fees(
            &source,
            &token,
            &symbol_short!("swap"),
            &1_000_000,
            &lp_dest,
            &protocol_dest,
            &backstop_dest,
            &platform_dest,
        );

        // Fees were actually routed from source to each destination.
        assert_eq!(MockTokenClient::new(&env, &token).balance(&source), 0);
        assert_eq!(MockTokenClient::new(&env, &token).balance(&lp_dest), 500_000);
        assert_eq!(MockTokenClient::new(&env, &token).balance(&protocol_dest), 200_000);
        assert_eq!(MockTokenClient::new(&env, &token).balance(&backstop_dest), 200_000);
        assert_eq!(MockTokenClient::new(&env, &token).balance(&platform_dest), 100_000);
    }
}
