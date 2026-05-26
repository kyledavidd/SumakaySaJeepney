#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    Address, Env, Map, Symbol, Vec,
};

// ─── Storage Key Types ───────────────────────────────────────────────────────

/// Identifies the jeepney operator/driver who owns this contract instance.
/// Stored once at initialization.
const DRIVER: Symbol = symbol_short!("DRIVER");

/// A running total of all fares received (in stroops, 1 XLM = 10_000_000 stroops).
const TOTAL_FARE: Symbol = symbol_short!("TOTAL");

/// A running count of total passengers who have paid.
const PASS_COUNT: Symbol = symbol_short!("PASSCNT");

/// Ledger key prefix for per-ride payment records.
const PAYMENTS: Symbol = symbol_short!("PAYMENTS");

// ─── Data Structures ─────────────────────────────────────────────────────────

/// A record of a single fare payment.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FarePayment {
    /// The wallet address of the passenger who paid.
    pub passenger: Address,
    /// Amount paid in stroops (1 XLM = 10_000_000 stroops).
    pub amount: i128,
    /// Ledger timestamp at the time of payment.
    pub timestamp: u64,
    /// Short route description, e.g. "SM–Dolores".
    pub route: soroban_sdk::String,
}

/// Summary statistics for the driver's dashboard.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DriverStats {
    pub total_collected: i128,
    pub passenger_count: u32,
}

// ─── Contract ────────────────────────────────────────────────────────────────

#[contract]
pub struct JeepneyFareContract;

#[contractimpl]
impl JeepneyFareContract {
    // ── Initialize ─────────────────────────────────────────────────────────

    /// Initialize the contract with the driver's wallet address.
    /// Must be called once before any payments can be processed.
    /// `driver` – the jeepney operator's Stellar account address.
    pub fn initialize(env: Env, driver: Address) {
        // Prevent re-initialization: check if DRIVER key already exists.
        if env.storage().instance().has(&DRIVER) {
            panic!("Contract already initialized");
        }

        // Persist the driver address.
        env.storage().instance().set(&DRIVER, &driver);

        // Initialize counters to zero.
        env.storage().instance().set(&TOTAL_FARE, &0_i128);
        env.storage().instance().set(&PASS_COUNT, &0_u32);
    }

    // ── Pay Fare ───────────────────────────────────────────────────────────

    /// Called when a passenger scans the QR code and submits their fare.
    /// Records the payment on-chain and credits the driver's running total.
    ///
    /// `passenger` – the passenger's Stellar address (must authorize).
    /// `amount`    – fare amount in stroops (e.g. 1 XLM = 10_000_000).
    /// `route`     – short text describing the route segment paid for.
    ///
    /// Returns the sequential payment index for this ride session.
    pub fn pay_fare(
        env: Env,
        passenger: Address,
        amount: i128,
        route: soroban_sdk::String,
    ) -> u32 {
        // Require the passenger's signature — prevents spoofed payments.
        passenger.require_auth();

        // Validate that the amount is positive.
        if amount <= 0 {
            panic!("Fare amount must be positive");
        }

        // Retrieve the current passenger count to use as the payment index.
        let mut count: u32 = env.storage().instance().get(&PASS_COUNT).unwrap_or(0);

        // Build the payment record.
        let payment = FarePayment {
            passenger: passenger.clone(),
            amount,
            timestamp: env.ledger().timestamp(),
            route,
        };

        // Store the individual payment record keyed by index.
        // We use a persistent map so each payment survives ledger expiry.
        let mut payments: Map<u32, FarePayment> = env
            .storage()
            .persistent()
            .get(&PAYMENTS)
            .unwrap_or(Map::new(&env));

        payments.set(count, payment);
        env.storage().persistent().set(&PAYMENTS, &payments);

        // Update the running totals.
        let mut total: i128 = env.storage().instance().get(&TOTAL_FARE).unwrap_or(0);
        total += amount;
        count += 1;

        env.storage().instance().set(&TOTAL_FARE, &total);
        env.storage().instance().set(&PASS_COUNT, &count);

        // Emit an event so the driver's mobile app can react in real time.
        env.events().publish(
            (symbol_short!("fare_paid"), passenger),
            amount,
        );

        // Return the 0-based index of this payment.
        count - 1
    }

    // ── Get Driver Stats ────────────────────────────────────────────────────

    /// Returns aggregate statistics for the driver: total XLM collected
    /// (in stroops) and number of paying passengers this session.
    pub fn get_stats(env: Env) -> DriverStats {
        DriverStats {
            total_collected: env.storage().instance().get(&TOTAL_FARE).unwrap_or(0),
            passenger_count: env.storage().instance().get(&PASS_COUNT).unwrap_or(0),
        }
    }

    // ── Get Single Payment ──────────────────────────────────────────────────

    /// Retrieve the full details of a specific fare payment by its index.
    /// Useful for receipts or audits.
    pub fn get_payment(env: Env, index: u32) -> FarePayment {
        let payments: Map<u32, FarePayment> = env
            .storage()
            .persistent()
            .get(&PAYMENTS)
            .unwrap_or(Map::new(&env));

        payments.get(index).expect("Payment index not found")
    }

    // ── Get Driver Address ──────────────────────────────────────────────────

    /// Returns the driver address stored at initialization.
    /// Useful for the frontend to display the QR code destination.
    pub fn get_driver(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DRIVER)
            .expect("Contract not initialized")
    }

    // ── Get All Payments ────────────────────────────────────────────────────

    /// Returns a vector of all payment records.
    /// Intended for driver's end-of-day earnings summary.
    pub fn get_all_payments(env: Env) -> Vec<FarePayment> {
        let count: u32 = env.storage().instance().get(&PASS_COUNT).unwrap_or(0);
        let payments: Map<u32, FarePayment> = env
            .storage()
            .persistent()
            .get(&PAYMENTS)
            .unwrap_or(Map::new(&env));

        let mut result = Vec::new(&env);
        for i in 0..count {
            if let Some(p) = payments.get(i) {
                result.push_back(p);
            }
        }
        result
    }
}