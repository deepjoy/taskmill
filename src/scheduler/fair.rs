//! Weighted fair scheduling — per-group slot allocation.
//!
//! When group weights are configured, the scheduler uses a three-pass
//! dispatch loop:
//!
//! 1. **Fair pass** — each group (including ungrouped tasks as a virtual
//!    group) receives slots proportional to its weight.
//! 2. **Greedy pass** — unfilled slots are filled by global priority order.
//! 3. **Urgent pass** — tasks aged past `urgent_threshold` bypass weights
//!    (but still respect `max_concurrency`).
//!
//! This module provides [`GroupWeights`] (thread-safe weight storage) and
//! [`compute_allocation`] (the per-cycle slot allocator).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use super::gate::GroupLimits;

// ── GroupWeights ────────────────────────────────────────────────────

/// Per-group scheduling weights for weighted fair dispatch.
///
/// Weights are relative — `(A:3, B:1)` gives A 75% and B 25% of capacity.
/// A group without an explicit weight gets `default_weight`.
///
/// Thread safety: `RwLock` — reads (every dispatch cycle) vastly outnumber
/// writes (runtime reconfiguration).
pub struct GroupWeights {
    default_weight: AtomicU32,
    weights: RwLock<HashMap<String, u32>>,
    min_slots: RwLock<HashMap<String, usize>>,
}

impl Default for GroupWeights {
    fn default() -> Self {
        Self::new()
    }
}

impl GroupWeights {
    pub fn new() -> Self {
        Self {
            default_weight: AtomicU32::new(1),
            weights: RwLock::new(HashMap::new()),
            min_slots: RwLock::new(HashMap::new()),
        }
    }

    /// The default weight used for groups without an explicit override
    /// and for the ungrouped virtual group in the allocation.
    pub fn default_weight(&self) -> u32 {
        self.default_weight.load(Ordering::Relaxed)
    }

    /// Effective weight for a group (override or default).
    pub fn weight_for(&self, group: &str) -> u32 {
        self.weights
            .read()
            .unwrap()
            .get(group)
            .copied()
            .unwrap_or_else(|| self.default_weight())
    }

    /// Minimum guaranteed slots for a group (`None` = no minimum).
    pub fn min_slots_for(&self, group: &str) -> Option<usize> {
        self.min_slots.read().unwrap().get(group).copied()
    }

    /// Set weight for a specific group.
    pub fn set_weight(&self, group: String, weight: u32) {
        self.weights.write().unwrap().insert(group, weight);
    }

    /// Remove per-group weight override.
    pub fn remove_weight(&self, group: &str) {
        self.weights.write().unwrap().remove(group);
    }

    /// Set minimum guaranteed slots for a group.
    pub fn set_min_slots(&self, group: String, slots: usize) {
        self.min_slots.write().unwrap().insert(group, slots);
    }

    /// Set the default weight for groups without overrides.
    pub fn set_default(&self, weight: u32) {
        self.default_weight.store(weight, Ordering::Relaxed);
    }

    /// Reset all weights to default.
    pub fn reset_all(&self) {
        self.weights.write().unwrap().clear();
        self.min_slots.write().unwrap().clear();
        self.default_weight.store(1, Ordering::Relaxed);
    }

    /// Returns `true` if any weights or min_slots are configured.
    pub fn is_configured(&self) -> bool {
        !self.weights.read().unwrap().is_empty() || !self.min_slots.read().unwrap().is_empty()
    }
}

// ── Slot Allocation ────────────────────────────────────────────────

/// Per-group slot allocation computed each dispatch cycle.
pub(crate) struct SlotAllocation {
    /// group_key → total slots (running + available-to-dispatch).
    /// `None` key = ungrouped tasks (treated as a virtual group with
    /// default weight in the allocation algorithm).
    pub groups: Vec<(Option<String>, usize)>,
}

/// Demand for a single group: how many tasks are running and pending.
pub(crate) struct GroupDemand {
    pub running: usize,
    pub pending: usize,
}

/// Compute per-group slot allocations.
///
/// `groups` uses `Option<String>` keys — `None` = ungrouped tasks,
/// treated as a virtual group with default weight. This ensures
/// ungrouped tasks compete fairly rather than only receiving leftovers.
///
/// Algorithm:
/// 1. Guarantee `min_slots` per group (capped at demand).
/// 2. Distribute remaining capacity proportional to weights.
/// 3. Apply group concurrency caps.
/// 4. Work-conserving: excess from capped/drained groups redistributed.
pub(crate) fn compute_allocation(
    total_capacity: usize,
    groups: &[(Option<String>, GroupDemand)],
    weights: &GroupWeights,
    caps: &GroupLimits,
    paused_groups: &std::collections::HashSet<String>,
) -> SlotAllocation {
    // Filter out paused groups (release their allocation).
    // The ungrouped virtual group (key = None) is never paused.
    let active: Vec<_> = groups
        .iter()
        .filter(|(g, _)| match g {
            Some(key) => !paused_groups.contains(key),
            None => true,
        })
        .collect();

    let mut alloc: HashMap<Option<String>, usize> = HashMap::new();
    let mut remaining = total_capacity;

    // Step 1: Guarantee minimums (ungrouped group has no minimum).
    for (group, demand) in &active {
        let min = match group {
            Some(key) => weights.min_slots_for(key).unwrap_or(0),
            None => 0,
        };
        let need = demand.running + demand.pending;
        let grant = min.min(need).min(remaining);
        alloc.insert((*group).clone(), grant);
        remaining -= grant;
    }

    // Step 2: Distribute remaining by weight.
    // The ungrouped virtual group gets the default weight.
    let total_weight: u32 = active
        .iter()
        .map(|(g, _)| match g {
            Some(key) => weights.weight_for(key),
            None => weights.default_weight(),
        })
        .sum();

    if total_weight > 0 && remaining > 0 {
        // Compute raw weight-proportional shares, then iteratively
        // redistribute from over-allocated groups (bounded by demand
        // or cap) to under-allocated ones.
        struct GroupInfo {
            key: Option<String>,
            share: f64,
            need: usize,
            cap: usize,
        }

        let mut infos: Vec<GroupInfo> = active
            .iter()
            .map(|(g, demand)| {
                let w = match g {
                    Some(key) => weights.weight_for(key) as f64,
                    None => weights.default_weight() as f64,
                };
                let already = *alloc.get(g).unwrap_or(&0);
                let need = (demand.running + demand.pending).saturating_sub(already);
                let cap = match g {
                    Some(key) => caps.limit_for(key).unwrap_or(usize::MAX),
                    None => usize::MAX,
                };
                let cap_headroom = cap.saturating_sub(already);
                let share = remaining as f64 * w / total_weight as f64;
                GroupInfo {
                    key: (*g).clone(),
                    share: share.min(need as f64).min(cap_headroom as f64).max(0.0),
                    need: need.min(cap_headroom),
                    cap: cap_headroom,
                }
            })
            .collect();

        // Iteratively redistribute: if bounded shares sum to less than
        // remaining, redistribute the surplus to groups with headroom.
        for _ in 0..10 {
            let floored_sum: usize = infos.iter().map(|i| i.share as usize).sum();
            let surplus = remaining.saturating_sub(floored_sum);
            if surplus == 0 {
                break;
            }
            let mut redistributed = false;
            for info in infos.iter_mut() {
                let floored = info.share as usize;
                let headroom = info.need.saturating_sub(floored);
                if headroom > 0 && info.share < info.need as f64 {
                    let extra = (surplus as f64).min(headroom as f64);
                    info.share = (info.share + extra).min(info.need as f64);
                    redistributed = true;
                    break;
                }
            }
            if !redistributed {
                break;
            }
        }

        // Largest-remainder rounding.
        let floored_sum: usize = infos.iter().map(|i| i.share as usize).sum();
        let mut leftover = remaining.saturating_sub(floored_sum);

        // Sort by fractional part descending for largest-remainder.
        // Tie-break by group name (None < Some) for deterministic allocation.
        infos.sort_by(|a, b| {
            let fa = a.share - a.share.floor();
            let fb = b.share - b.share.floor();
            fb.partial_cmp(&fa)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.key.cmp(&b.key))
        });
        for info in &infos {
            let floored = info.share as usize;
            let can_take = info
                .need
                .saturating_sub(floored)
                .min(info.cap.saturating_sub(floored));
            let extra = if leftover > 0 && can_take > 0 {
                leftover -= 1;
                1
            } else {
                0
            };
            *alloc.entry(info.key.clone()).or_default() += floored + extra;
        }
    }

    SlotAllocation {
        groups: alloc.into_iter().collect(),
    }
}

// ── Snapshot Info ──────────────────────────────────────────────────

/// Per-group allocation info for snapshot/dashboard display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupAllocationInfo {
    pub group: String,
    pub weight: u32,
    pub allocated_slots: usize,
    pub running: usize,
    pub pending: usize,
    pub min_slots: Option<usize>,
    pub cap: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_demand(
        groups: Vec<(Option<String>, usize, usize)>,
    ) -> Vec<(Option<String>, GroupDemand)> {
        groups
            .into_iter()
            .map(|(g, running, pending)| (g, GroupDemand { running, pending }))
            .collect()
    }

    #[test]
    fn allocation_proportional_to_weights() {
        let weights = GroupWeights::new();
        weights.set_weight("A".into(), 3);
        weights.set_weight("B".into(), 1);
        let caps = GroupLimits::new();
        let paused = std::collections::HashSet::new();

        let demand = make_demand(vec![(Some("A".into()), 0, 100), (Some("B".into()), 0, 100)]);

        let alloc = compute_allocation(8, &demand, &weights, &caps, &paused);
        let map: HashMap<_, _> = alloc.groups.into_iter().collect();
        assert_eq!(map[&Some("A".into())], 6); // 3/4 * 8 = 6
        assert_eq!(map[&Some("B".into())], 2); // 1/4 * 8 = 2
    }

    #[test]
    fn allocation_respects_min_slots() {
        let weights = GroupWeights::new();
        weights.set_weight("A".into(), 10);
        weights.set_weight("B".into(), 1);
        weights.set_min_slots("B".into(), 3);
        let caps = GroupLimits::new();
        let paused = std::collections::HashSet::new();

        let demand = make_demand(vec![(Some("A".into()), 0, 100), (Some("B".into()), 0, 100)]);

        let alloc = compute_allocation(10, &demand, &weights, &caps, &paused);
        let map: HashMap<_, _> = alloc.groups.into_iter().collect();
        // B gets at least 3 from min_slots
        assert!(map[&Some("B".into())] >= 3);
        // Total should equal capacity
        let total: usize = map.values().sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn allocation_respects_caps() {
        let weights = GroupWeights::new();
        weights.set_weight("A".into(), 1);
        weights.set_weight("B".into(), 1);
        let caps = GroupLimits::new();
        caps.set_limit("A".into(), 2);
        let paused = std::collections::HashSet::new();

        let demand = make_demand(vec![(Some("A".into()), 0, 100), (Some("B".into()), 0, 100)]);

        let alloc = compute_allocation(10, &demand, &weights, &caps, &paused);
        let map: HashMap<_, _> = alloc.groups.into_iter().collect();
        assert_eq!(map[&Some("A".into())], 2); // capped at 2
        assert_eq!(map[&Some("B".into())], 8); // gets the excess
    }

    #[test]
    fn allocation_work_conserving() {
        let weights = GroupWeights::new();
        weights.set_weight("A".into(), 1);
        weights.set_weight("B".into(), 1);
        let caps = GroupLimits::new();
        let paused = std::collections::HashSet::new();

        // A has only 2 pending, B has plenty
        let demand = make_demand(vec![(Some("A".into()), 0, 2), (Some("B".into()), 0, 100)]);

        let alloc = compute_allocation(10, &demand, &weights, &caps, &paused);
        let map: HashMap<_, _> = alloc.groups.into_iter().collect();
        assert_eq!(map[&Some("A".into())], 2); // only 2 needed
        assert_eq!(map[&Some("B".into())], 8); // gets the rest
    }

    #[test]
    fn paused_groups_excluded() {
        let weights = GroupWeights::new();
        weights.set_weight("A".into(), 1);
        weights.set_weight("B".into(), 1);
        let caps = GroupLimits::new();
        let mut paused = std::collections::HashSet::new();
        paused.insert("A".into());

        let demand = make_demand(vec![(Some("A".into()), 0, 100), (Some("B".into()), 0, 100)]);

        let alloc = compute_allocation(10, &demand, &weights, &caps, &paused);
        let map: HashMap<_, _> = alloc.groups.into_iter().collect();
        assert!(!map.contains_key(&Some("A".into())));
        assert_eq!(map[&Some("B".into())], 10);
    }

    #[test]
    fn equal_weights_equal_allocation() {
        let weights = GroupWeights::new();
        weights.set_weight("A".into(), 1);
        weights.set_weight("B".into(), 1);
        let caps = GroupLimits::new();
        let paused = std::collections::HashSet::new();

        let demand = make_demand(vec![(Some("A".into()), 0, 100), (Some("B".into()), 0, 100)]);

        let alloc = compute_allocation(10, &demand, &weights, &caps, &paused);
        let map: HashMap<_, _> = alloc.groups.into_iter().collect();
        assert_eq!(map[&Some("A".into())], 5);
        assert_eq!(map[&Some("B".into())], 5);
    }

    #[test]
    fn single_group_gets_all() {
        let weights = GroupWeights::new();
        weights.set_weight("A".into(), 1);
        let caps = GroupLimits::new();
        let paused = std::collections::HashSet::new();

        let demand = make_demand(vec![(Some("A".into()), 0, 100)]);

        let alloc = compute_allocation(10, &demand, &weights, &caps, &paused);
        let map: HashMap<_, _> = alloc.groups.into_iter().collect();
        assert_eq!(map[&Some("A".into())], 10);
    }

    #[test]
    fn zero_pending_gets_zero() {
        let weights = GroupWeights::new();
        weights.set_weight("A".into(), 1);
        weights.set_weight("B".into(), 1);
        let caps = GroupLimits::new();
        let paused = std::collections::HashSet::new();

        let demand = make_demand(vec![(Some("A".into()), 0, 0), (Some("B".into()), 0, 100)]);

        let alloc = compute_allocation(10, &demand, &weights, &caps, &paused);
        let map: HashMap<_, _> = alloc.groups.into_iter().collect();
        assert_eq!(map.get(&Some("A".into())).copied().unwrap_or(0), 0);
        assert_eq!(map[&Some("B".into())], 10);
    }

    #[test]
    fn ungrouped_gets_default_weight() {
        let weights = GroupWeights::new(); // default weight = 1
        weights.set_weight("A".into(), 1);
        let caps = GroupLimits::new();
        let paused = std::collections::HashSet::new();

        let demand = make_demand(vec![
            (Some("A".into()), 0, 100),
            (None, 0, 100), // ungrouped
        ]);

        let alloc = compute_allocation(10, &demand, &weights, &caps, &paused);
        let map: HashMap<_, _> = alloc.groups.into_iter().collect();
        assert_eq!(map[&Some("A".into())], 5);
        assert_eq!(map[&None], 5);
    }

    #[test]
    fn sum_allocations_equals_capacity() {
        let weights = GroupWeights::new();
        weights.set_weight("A".into(), 3);
        weights.set_weight("B".into(), 2);
        weights.set_weight("C".into(), 1);
        let caps = GroupLimits::new();
        let paused = std::collections::HashSet::new();

        let demand = make_demand(vec![
            (Some("A".into()), 0, 100),
            (Some("B".into()), 0, 100),
            (Some("C".into()), 0, 100),
        ]);

        let alloc = compute_allocation(10, &demand, &weights, &caps, &paused);
        let total: usize = alloc.groups.iter().map(|(_, s)| s).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn largest_remainder_deterministic() {
        // With 3 groups of equal weight and 10 slots, we get 3.33 each.
        // Largest-remainder should give 4, 3, 3 deterministically.
        let weights = GroupWeights::new();
        weights.set_weight("A".into(), 1);
        weights.set_weight("B".into(), 1);
        weights.set_weight("C".into(), 1);
        let caps = GroupLimits::new();
        let paused = std::collections::HashSet::new();

        let demand = make_demand(vec![
            (Some("A".into()), 0, 100),
            (Some("B".into()), 0, 100),
            (Some("C".into()), 0, 100),
        ]);

        // Run twice — must produce identical results.
        let alloc1 = compute_allocation(10, &demand, &weights, &caps, &paused);
        let alloc2 = compute_allocation(10, &demand, &weights, &caps, &paused);
        let mut map1: Vec<_> = alloc1.groups.into_iter().collect();
        let mut map2: Vec<_> = alloc2.groups.into_iter().collect();
        map1.sort_by(|a, b| a.0.cmp(&b.0));
        map2.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(map1, map2);
        let total: usize = map1.iter().map(|(_, s)| s).sum();
        assert_eq!(total, 10);
    }
}
