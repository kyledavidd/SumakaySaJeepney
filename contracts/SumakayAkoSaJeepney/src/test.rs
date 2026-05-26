#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger, LedgerInfo},
        Address, Env, String,
    };

    use crate::{JeepneyFareContract, JeepneyFareContractClient};

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Deploys a fresh contract instance and returns (env, client, driver_address).
    fn setup() -> (Env, JeepneyFareContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();

        // Set a deterministic ledger timestamp so tests are reproducible.
        env.ledger().set(LedgerInfo {
            timestamp: 1_700_000_000,
            protocol_version: 20,
            sequence_number: 100,
            network_id: Default::default(),
            base_reserve: 5_000_000,
            min_temp_entry_ttl: 16,
            min_persistent_entry_ttl: 50000,
            max_entry_ttl: 9999999,
        });

        let contract_id = env.register_contract(None, JeepneyFareContract);
        let client = JeepneyFareContractClient::new(&env, &contract_id);

        let driver = Address::generate(&env);
        client.initialize(&driver);

        (env, client, driver)
    }

    // ── Test 1: Happy Path ────────────────────────────────────────────────────
    // A passenger successfully pays a fare end-to-end.
    // Verifies: the function returns index 0, no panics, stats update correctly.
    #[test]
    fn test_pay_fare_happy_path() {
        let (env, client, _driver) = setup();

        let passenger = Address::generate(&env);
        // 13 pesos worth — roughly 0.13 XLM at a notional rate.
        // Represented as 1_300_000 stroops (0.13 XLM).
        let fare_amount: i128 = 1_300_000;
        let route = String::from_str(&env, "SM Pampanga–Dolores");

        let payment_index = client.pay_fare(&passenger, &fare_amount, &route);

        // The very first payment should be at index 0.
        assert_eq!(payment_index, 0);

        // Stats should now reflect one passenger and the correct total.
        let stats = client.get_stats();
        assert_eq!(stats.passenger_count, 1);
        assert_eq!(stats.total_collected, fare_amount);
    }

    // ── Test 2: Edge Case – Zero / Negative Fare Rejected ────────────────────
    // A passenger attempting to pay 0 stroops should cause a panic.
    // Verifies: the contract enforces positive-amount validation.
    #[test]
    #[should_panic(expected = "Fare amount must be positive")]
    fn test_pay_fare_zero_amount_rejected() {
        let (env, client, _driver) = setup();

        let passenger = Address::generate(&env);
        let bad_amount: i128 = 0;
        let route = String::from_str(&env, "Nepo Mall–City Hall");

        // This must panic because amount == 0.
        client.pay_fare(&passenger, &bad_amount, &route);
    }

    // ── Test 3: State Verification ────────────────────────────────────────────
    // After three passengers pay, storage must reflect the correct cumulative state.
    // Verifies: passenger_count == 3, total_collected == sum of all fares.
    #[test]
    fn test_state_after_multiple_payments() {
        let (env, client, _driver) = setup();

        let fares: [i128; 3] = [1_300_000, 1_500_000, 1_000_000]; // in stroops
        let route = String::from_str(&env, "Dolores–San Fernando City Hall");

        for fare in fares.iter() {
            let passenger = Address::generate(&env);
            client.pay_fare(&passenger, fare, &route);
        }

        let stats = client.get_stats();
        assert_eq!(stats.passenger_count, 3);
        assert_eq!(
            stats.total_collected,
            fares.iter().sum::<i128>(),
            "Total collected must equal the sum of all fares"
        );
    }

    // ── Test 4: Payment Record Retrieval ──────────────────────────────────────
    // After paying, the payment record can be retrieved by index with correct data.
    // Verifies: get_payment returns the correct passenger address, amount, and route.
    #[test]
    fn test_get_payment_record_correct() {
        let (env, client, _driver) = setup();

        let passenger = Address::generate(&env);
        let fare_amount: i128 = 2_000_000; // 0.20 XLM
        let route = String::from_str(&env, "Sindalan–City Center");

        let index = client.pay_fare(&passenger, &fare_amount, &route);

        let record = client.get_payment(&index);

        assert_eq!(record.passenger, passenger, "Passenger address must match");
        assert_eq!(record.amount, fare_amount, "Amount must match");
        assert_eq!(record.route, route, "Route must match");
    }

    // ── Test 5: Double Initialization Rejected ────────────────────────────────
    // Calling initialize() a second time must panic to prevent driver replacement.
    // Verifies: the guard clause "Contract already initialized" fires correctly.
    #[test]
    #[should_panic(expected = "Contract already initialized")]
    fn test_double_initialization_rejected() {
        let (env, client, _driver) = setup();

        // Try to initialize a second time with a different driver — must panic.
        let attacker = Address::generate(&env);
        client.initialize(&attacker);
    }
}