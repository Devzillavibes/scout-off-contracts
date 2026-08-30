//! CPU-instruction cost regression budget for the verification contract.
//!
//! Measures the CPU-instruction cost of representative verification
//! operations using soroban-sdk's test budget utilities (`Env::cost_estimate`)
//! and asserts each stays within a checked-in per-operation budget. See
//! `ci/cpu-cost-budget.md` for the full cross-contract budget table and the
//! process for raising a budget when a legitimate feature grows an
//! operation's cost.
//!
//! To raise a budget: bump the relevant constant below AND update the
//! matching row in `ci/cpu-cost-budget.md` with a one-line justification in
//! the PR description explaining why the growth is expected and acceptable.

use scoutchain_verification::{VerificationContract, VerificationContractClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

// These starting budgets are deliberately generous placeholders, not
// measured baselines: this environment could not run `cargo test` to
// capture real current costs when this file was first introduced (no Rust
// toolchain available). Tighten each budget to roughly
// current-cost-plus-headroom after the first real CI run reports actual
// numbers — that tightening is a follow-up, not a blocker.
const REGISTER_VALIDATOR_CPU_BUDGET: u64 = 15_000_000;
const APPROVE_MILESTONE_CPU_BUDGET: u64 = 20_000_000;
const GET_VALIDATOR_MILESTONES_PAGE_CPU_BUDGET: u64 = 15_000_000;

// Distinct valid CIDv0 evidence hashes (exactly 46 chars, base58btc — no
// 0/O/I/l). approve_milestone rejects duplicate evidence hashes globally.
const CID_1: &str = "QmRhbYsqpiYgUY9KfNCcbfopHPbLnWSVKBpDNs37aZ3kVC";
const CID_2: &str = "QmwsjoZwgfzgx6xPr3cXEKhzfLt5RQ87yMnWecTp1tf6p7";
const CID_3: &str = "QmgzsER5ykyxoTsVUSePRkKXqkEzsRVLpUv511dp4c3vAs";

fn setup() -> (Env, VerificationContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VerificationContract, ());
    let client = VerificationContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client)
}

/// Reads the CPU-instruction cost accumulated since the last budget reset
/// and asserts it is within `budget`, panicking with a diagnostic naming the
/// operation, the measured cost, and the overage when it is not.
fn assert_cpu_budget(env: &Env, op: &str, budget: u64) {
    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    println!("cost_budget: verification::{op} = {cpu} cpu instructions (budget {budget})");
    assert!(
        cpu <= budget,
        "verification::{op} regressed: measured {cpu} cpu instructions, exceeding the \
         {budget}-instruction budget by {over} ({pct:.1}% over). See ci/cpu-cost-budget.md \
         for how to raise this budget if the growth is intentional.",
        over = cpu.saturating_sub(budget),
        pct = (cpu.saturating_sub(budget)) as f64 / budget as f64 * 100.0,
    );
}

#[test]
fn cost_register_validator() {
    let (env, client) = setup();
    let validator = Address::generate(&env);
    let credentials = String::from_str(&env, "UEFA-A-License-2026");

    env.cost_estimate().budget().reset_default();
    client.register_validator(&validator, &credentials);
    assert_cpu_budget(&env, "register_validator", REGISTER_VALIDATOR_CPU_BUDGET);
}

#[test]
fn cost_approve_milestone() {
    let (env, client) = setup();
    let validator = Address::generate(&env);
    client.register_validator(&validator, &String::from_str(&env, "UEFA-A-License-2026"));

    env.cost_estimate().budget().reset_default();
    client.approve_milestone(
        &validator,
        &1u64,
        &String::from_str(&env, "scored a hat-trick"),
        &String::from_str(&env, CID_1),
    );
    assert_cpu_budget(&env, "approve_milestone", APPROVE_MILESTONE_CPU_BUDGET);
}

#[test]
fn cost_get_validator_milestones_page() {
    let (env, client) = setup();
    let validator = Address::generate(&env);
    client.register_validator(&validator, &String::from_str(&env, "UEFA-A-License-2026"));
    client.approve_milestone(
        &validator,
        &1u64,
        &String::from_str(&env, "scored a hat-trick"),
        &String::from_str(&env, CID_2),
    );
    client.approve_milestone(
        &validator,
        &2u64,
        &String::from_str(&env, "clean sheet"),
        &String::from_str(&env, CID_3),
    );

    env.cost_estimate().budget().reset_default();
    client.get_validator_milestones_page(&validator, &0u32, &10u32);
    assert_cpu_budget(
        &env,
        "get_validator_milestones_page",
        GET_VALIDATOR_MILESTONES_PAGE_CPU_BUDGET,
    );
}

// ── pagination guard budget tests (issue #1162) ──────────────────────────────

const LIST_DISPUTES_PAGE_CPU_BUDGET: u64 = 15_000_000;
const GET_GLOBAL_MILESTONE_INDEX_CPU_BUDGET: u64 = 15_000_000;
const GET_MILESTONES_SINCE_PAGE_CPU_BUDGET: u64 = 20_000_000;

// Distinct valid CIDv0 hashes for dispute budget test (different from the
// three already used above to avoid DuplicateEvidence errors).
const CID_D1: &str = "QmcpnqFWJhCr5Ys36TQcjzGPJWqFdNRPGjzHwmPV4bJZ9";
const CID_D2: &str = "QmYmrMkD9Bc5DBUCuAMJVTfVQVxG2uxNJCVAzmqAuPyFa8";

/// Asserts that list_disputes_page returns empty immediately when offset >= len
/// without iterating the entire index (DoS guard).  The budget is measured at
/// the worst-case starting point (offset == 0, full 50-entry page) to show the
/// loop itself is bounded.
#[test]
fn cost_list_disputes_page_bounded() {
    let (env, client) = setup();
    let validator = Address::generate(&env);
    client.register_validator(&validator, &String::from_str(&env, "UEFA-A-License-2026"));

    // Approve two milestones so there are disputes to file.
    client.approve_milestone(
        &validator,
        &1u64,
        &String::from_str(&env, "scored in cup"),
        &String::from_str(&env, CID_D1),
    );
    client.approve_milestone(
        &validator,
        &2u64,
        &String::from_str(&env, "assists record"),
        &String::from_str(&env, CID_D2),
    );

    // Verify that a past-the-end offset returns empty immediately.
    let page_past_end = client.list_disputes_page(&100u32, &50u32);
    assert_eq!(page_past_end.len(), 0);

    // Measure the normal case (offset=0).
    env.cost_estimate().budget().reset_default();
    client.list_disputes_page(&0u32, &50u32);
    assert_cpu_budget(&env, "list_disputes_page", LIST_DISPUTES_PAGE_CPU_BUDGET);
}

/// Asserts that get_global_milestone_index returns empty immediately when
/// offset >= total, and that the full first-page cost is within budget.
#[test]
fn cost_get_global_milestone_index_bounded() {
    let (env, client) = setup();
    let validator = Address::generate(&env);
    client.register_validator(&validator, &String::from_str(&env, "UEFA-A-License-2026"));
    client.approve_milestone(
        &validator,
        &3u64,
        &String::from_str(&env, "dribble record"),
        &String::from_str(&env, CID_1),
    );

    // Past-the-end offset must return empty immediately.
    let result = client.get_global_milestone_index(&9999u32, &50u32);
    assert_eq!(result.entries.len(), 0);

    // Measure normal case.
    env.cost_estimate().budget().reset_default();
    client.get_global_milestone_index(&0u32, &50u32);
    assert_cpu_budget(
        &env,
        "get_global_milestone_index",
        GET_GLOBAL_MILESTONE_INDEX_CPU_BUDGET,
    );
}

/// Asserts that get_milestones_since_page returns empty for an out-of-bounds
/// offset and that the bounded page cost stays within budget.
#[test]
fn cost_get_milestones_since_page_bounded() {
    let (env, client) = setup();
    let validator = Address::generate(&env);
    client.register_validator(&validator, &String::from_str(&env, "UEFA-A-License-2026"));
    client.approve_milestone(
        &validator,
        &5u64,
        &String::from_str(&env, "speed record"),
        &String::from_str(&env, CID_2),
    );
    client.approve_milestone(
        &validator,
        &5u64,
        &String::from_str(&env, "endurance record"),
        &String::from_str(&env, CID_3),
    );

    // Past-the-end offset must return empty.
    let empty = client.get_milestones_since_page(&5u64, &0u64, &9999u32, &50u32);
    assert_eq!(empty.len(), 0);

    // Measure normal case (since=0 catches all milestones).
    env.cost_estimate().budget().reset_default();
    client.get_milestones_since_page(&5u64, &0u64, &0u32, &50u32);
    assert_cpu_budget(
        &env,
        "get_milestones_since_page",
        GET_MILESTONES_SINCE_PAGE_CPU_BUDGET,
    );
}
