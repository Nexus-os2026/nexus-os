//! Stripe-ready payment integration: plans, invoices, payouts, and revenue sharing.
//!
//! This module implements the data model and business logic for marketplace payments.
//! No Stripe SDK — this is the layer that Stripe (or any payment processor) plugs into.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Billing interval for a subscription plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BillingInterval {
    Monthly,
    Yearly,
    OneTime,
}

/// A subscription or one-time payment plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentPlan {
    pub id: String,
    pub name: String,
    pub price_cents: u64,
    pub interval: BillingInterval,
    pub features: Vec<String>,
}

/// Status of an invoice.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InvoiceStatus {
    Pending,
    Paid,
    Failed,
    Refunded,
}

/// An invoice generated for a plan purchase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub plan_id: String,
    pub buyer_id: String,
    pub amount_cents: u64,
    pub status: InvoiceStatus,
    pub created_at: u64,
    pub paid_at: Option<u64>,
}

/// A payout record for an agent developer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeveloperPayout {
    pub id: String,
    pub developer_id: String,
    pub amount_cents: u64,
    pub agent_id: String,
    pub period: String,
    pub created_at: u64,
}

/// Revenue split configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueSplit {
    /// Developer share as a percentage (0-100). Default: 70.
    pub developer_pct: u64,
    /// Platform share as a percentage (0-100). Default: 30.
    pub platform_pct: u64,
}

impl Default for RevenueSplit {
    fn default() -> Self {
        Self {
            developer_pct: 70,
            platform_pct: 30,
        }
    }
}

/// Aggregate revenue statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueStats {
    pub total_revenue_cents: u64,
    pub total_paid_invoices: usize,
    pub total_pending_invoices: usize,
    pub developer_share_cents: u64,
    pub platform_share_cents: u64,
    pub total_payouts_cents: u64,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Core payment engine managing plans, invoices, payouts, and revenue sharing.
#[derive(Debug, Clone)]
pub struct PaymentEngine {
    plans: HashMap<String, PaymentPlan>,
    invoices: Vec<Invoice>,
    payouts: Vec<DeveloperPayout>,
    /// Maps agent_id to developer_id for revenue routing.
    agent_developers: HashMap<String, String>,
    /// Maps plan_id to agent_id for revenue attribution.
    plan_agents: HashMap<String, String>,
    revenue_split: RevenueSplit,
}

impl PaymentEngine {
    pub fn new(revenue_split: RevenueSplit) -> Self {
        Self {
            plans: HashMap::new(),
            invoices: Vec::new(),
            payouts: Vec::new(),
            agent_developers: HashMap::new(),
            plan_agents: HashMap::new(),
            revenue_split,
        }
    }

    /// Register which developer owns an agent (for revenue routing).
    pub fn register_agent_developer(&mut self, agent_id: &str, developer_id: &str) {
        self.agent_developers
            .insert(agent_id.to_string(), developer_id.to_string());
    }

    /// Register which agent a plan belongs to (for revenue attribution).
    pub fn register_plan_agent(&mut self, plan_id: &str, agent_id: &str) {
        self.plan_agents
            .insert(plan_id.to_string(), agent_id.to_string());
    }

    /// Create a new payment plan.
    pub fn create_plan(
        &mut self,
        name: &str,
        price_cents: u64,
        interval: BillingInterval,
        features: Vec<String>,
    ) -> PaymentPlan {
        let plan = PaymentPlan {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            price_cents,
            interval,
            features,
        };
        self.plans.insert(plan.id.clone(), plan.clone());
        plan
    }

    /// List all available plans.
    pub fn list_plans(&self) -> Vec<&PaymentPlan> {
        self.plans.values().collect()
    }

    /// Get a plan by ID.
    pub fn get_plan(&self, plan_id: &str) -> Option<&PaymentPlan> {
        self.plans.get(plan_id)
    }

    /// Delete a plan by ID. Returns whether it existed.
    pub fn delete_plan(&mut self, plan_id: &str) -> bool {
        self.plans.remove(plan_id).is_some()
    }

    /// Create an invoice for a buyer purchasing a plan.
    pub fn create_invoice(&mut self, plan_id: &str, buyer_id: &str) -> Result<Invoice, String> {
        let plan = self
            .plans
            .get(plan_id)
            .ok_or_else(|| format!("plan not found: {plan_id}"))?;

        let invoice = Invoice {
            id: Uuid::new_v4().to_string(),
            plan_id: plan_id.to_string(),
            buyer_id: buyer_id.to_string(),
            amount_cents: plan.price_cents,
            status: InvoiceStatus::Pending,
            created_at: now_ms(),
            paid_at: None,
        };
        self.invoices.push(invoice.clone());
        Ok(invoice)
    }

    /// Mark an invoice as paid. Returns the updated invoice.
    pub fn pay_invoice(&mut self, invoice_id: &str) -> Result<Invoice, String> {
        let invoice = self
            .invoices
            .iter_mut()
            .find(|i| i.id == invoice_id)
            .ok_or_else(|| format!("invoice not found: {invoice_id}"))?;

        if invoice.status != InvoiceStatus::Pending {
            return Err(format!(
                "invoice {} is {:?}, not Pending",
                invoice_id, invoice.status
            ));
        }

        invoice.status = InvoiceStatus::Paid;
        invoice.paid_at = Some(now_ms());
        Ok(invoice.clone())
    }

    /// Refund a paid invoice.
    pub fn refund_invoice(&mut self, invoice_id: &str) -> Result<Invoice, String> {
        let invoice = self
            .invoices
            .iter_mut()
            .find(|i| i.id == invoice_id)
            .ok_or_else(|| format!("invoice not found: {invoice_id}"))?;

        if invoice.status != InvoiceStatus::Paid {
            return Err(format!(
                "invoice {} is {:?}, not Paid",
                invoice_id, invoice.status
            ));
        }

        invoice.status = InvoiceStatus::Refunded;
        Ok(invoice.clone())
    }

    /// Get an invoice by ID.
    pub fn get_invoice(&self, invoice_id: &str) -> Option<&Invoice> {
        self.invoices.iter().find(|i| i.id == invoice_id)
    }

    /// List invoices, optionally filtered by buyer.
    pub fn list_invoices(&self, buyer_id: Option<&str>) -> Vec<&Invoice> {
        match buyer_id {
            Some(bid) => self.invoices.iter().filter(|i| i.buyer_id == bid).collect(),
            None => self.invoices.iter().collect(),
        }
    }

    /// Calculate the developer's share of a given amount in cents (rounded to nearest).
    pub fn developer_share(&self, amount_cents: u64) -> u64 {
        (amount_cents * self.revenue_split.developer_pct + 50) / 100 // round to nearest
    }

    /// Calculate the platform's share of a given amount in cents (remainder ensures no loss).
    pub fn platform_share(&self, amount_cents: u64) -> u64 {
        amount_cents - self.developer_share(amount_cents) // remainder ensures no loss
    }

    /// Create a payout for a developer from agent revenue.
    pub fn create_payout(
        &mut self,
        developer_id: &str,
        agent_id: &str,
        amount_cents: u64,
        period: &str,
    ) -> DeveloperPayout {
        let payout = DeveloperPayout {
            id: Uuid::new_v4().to_string(),
            developer_id: developer_id.to_string(),
            amount_cents,
            agent_id: agent_id.to_string(),
            period: period.to_string(),
            created_at: now_ms(),
        };
        self.payouts.push(payout.clone());
        payout
    }

    /// Calculate and create payouts for all paid invoices in a period for a given agent.
    /// Returns the developer payout created (if any revenue exists).
    pub fn settle_agent_revenue(
        &mut self,
        agent_id: &str,
        period: &str,
    ) -> Option<DeveloperPayout> {
        let developer_id = self.agent_developers.get(agent_id)?.clone();

        // Sum paid invoices for plans associated with this agent.
        let total_paid: u64 = self
            .invoices
            .iter()
            .filter(|i| {
                i.status == InvoiceStatus::Paid
                    && self
                        .plan_agents
                        .get(&i.plan_id)
                        .is_some_and(|aid| aid == agent_id)
            })
            .map(|i| i.amount_cents)
            .sum();

        if total_paid == 0 {
            return None;
        }

        let dev_share = self.developer_share(total_paid);
        Some(self.create_payout(&developer_id, agent_id, dev_share, period))
    }

    /// List payouts for a developer.
    pub fn list_payouts(&self, developer_id: Option<&str>) -> Vec<&DeveloperPayout> {
        match developer_id {
            Some(did) => self
                .payouts
                .iter()
                .filter(|p| p.developer_id == did)
                .collect(),
            None => self.payouts.iter().collect(),
        }
    }

    /// Get aggregate revenue statistics.
    pub fn get_revenue_stats(&self) -> RevenueStats {
        let paid: Vec<&Invoice> = self
            .invoices
            .iter()
            .filter(|i| i.status == InvoiceStatus::Paid)
            .collect();
        let pending_count = self
            .invoices
            .iter()
            .filter(|i| i.status == InvoiceStatus::Pending)
            .count();

        let total_revenue: u64 = paid.iter().map(|i| i.amount_cents).sum();
        let total_payouts: u64 = self.payouts.iter().map(|p| p.amount_cents).sum();

        RevenueStats {
            total_revenue_cents: total_revenue,
            total_paid_invoices: paid.len(),
            total_pending_invoices: pending_count,
            developer_share_cents: self.developer_share(total_revenue),
            platform_share_cents: self.platform_share(total_revenue),
            total_payouts_cents: total_payouts,
        }
    }

    /// Get the current revenue split configuration.
    pub fn get_revenue_split(&self) -> &RevenueSplit {
        &self.revenue_split
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> PaymentEngine {
        PaymentEngine::new(RevenueSplit::default())
    }

    #[test]
    fn test_create_and_list_plans() {
        let mut e = engine();
        let p = e.create_plan(
            "Pro",
            999,
            BillingInterval::Monthly,
            vec!["unlimited-agents".into()],
        );
        assert_eq!(p.name, "Pro");
        assert_eq!(p.price_cents, 999);
        assert_eq!(e.list_plans().len(), 1);
    }

    #[test]
    fn test_get_and_delete_plan() {
        let mut e = engine();
        let p = e.create_plan("Basic", 499, BillingInterval::Monthly, vec![]);
        assert!(e.get_plan(&p.id).is_some());
        assert!(e.delete_plan(&p.id));
        assert!(e.get_plan(&p.id).is_none());
        assert!(!e.delete_plan("nonexistent"));
    }

    #[test]
    fn test_create_invoice_for_plan() {
        let mut e = engine();
        let plan = e.create_plan("Team", 2999, BillingInterval::Yearly, vec![]);
        let inv = e.create_invoice(&plan.id, "buyer-1").unwrap();
        assert_eq!(inv.amount_cents, 2999);
        assert_eq!(inv.status, InvoiceStatus::Pending);
        assert!(inv.paid_at.is_none());
    }

    #[test]
    fn test_create_invoice_unknown_plan() {
        let mut e = engine();
        assert!(e.create_invoice("no-such-plan", "buyer-1").is_err());
    }

    #[test]
    fn test_pay_invoice() {
        let mut e = engine();
        let plan = e.create_plan("Pro", 999, BillingInterval::Monthly, vec![]);
        let inv = e.create_invoice(&plan.id, "buyer-1").unwrap();
        let paid = e.pay_invoice(&inv.id).unwrap();
        assert_eq!(paid.status, InvoiceStatus::Paid);
        assert!(paid.paid_at.is_some());
    }

    #[test]
    fn test_pay_already_paid_invoice() {
        let mut e = engine();
        let plan = e.create_plan("Pro", 999, BillingInterval::Monthly, vec![]);
        let inv = e.create_invoice(&plan.id, "buyer-1").unwrap();
        e.pay_invoice(&inv.id).unwrap();
        assert!(e.pay_invoice(&inv.id).is_err());
    }

    #[test]
    fn test_refund_invoice() {
        let mut e = engine();
        let plan = e.create_plan("Pro", 999, BillingInterval::Monthly, vec![]);
        let inv = e.create_invoice(&plan.id, "buyer-1").unwrap();
        e.pay_invoice(&inv.id).unwrap();
        let refunded = e.refund_invoice(&inv.id).unwrap();
        assert_eq!(refunded.status, InvoiceStatus::Refunded);
    }

    #[test]
    fn test_refund_unpaid_invoice_fails() {
        let mut e = engine();
        let plan = e.create_plan("Pro", 999, BillingInterval::Monthly, vec![]);
        let inv = e.create_invoice(&plan.id, "buyer-1").unwrap();
        assert!(e.refund_invoice(&inv.id).is_err());
    }

    #[test]
    fn test_revenue_split_70_30() {
        let e = engine();
        assert_eq!(e.developer_share(10000), 7000);
        assert_eq!(e.platform_share(10000), 3000);
        // Shares sum to total.
        assert_eq!(e.developer_share(10000) + e.platform_share(10000), 10000);
    }

    #[test]
    fn test_custom_revenue_split() {
        let e = PaymentEngine::new(RevenueSplit {
            developer_pct: 80,
            platform_pct: 20,
        });
        assert_eq!(e.developer_share(10000), 8000);
        assert_eq!(e.platform_share(10000), 2000);
    }

    #[test]
    fn test_revenue_split_edge_cases() {
        let e = engine();
        assert_eq!(e.developer_share(0), 0);
        assert_eq!(e.platform_share(0), 0);
        assert_eq!(e.developer_share(1), 1); // 70% of 1 cent = 1 (rounded to nearest)
        assert_eq!(e.developer_share(10), 7);
        assert_eq!(e.platform_share(10), 3);
    }

    #[test]
    fn test_create_payout() {
        let mut e = engine();
        let payout = e.create_payout("dev-1", "agent-x", 7000, "2026-03");
        assert_eq!(payout.developer_id, "dev-1");
        assert_eq!(payout.amount_cents, 7000);
        assert_eq!(payout.agent_id, "agent-x");
        assert_eq!(payout.period, "2026-03");
    }

    #[test]
    fn test_settle_agent_revenue() {
        let mut e = engine();
        e.register_agent_developer("agent-x", "dev-1");
        let plan = e.create_plan("Pro", 10000, BillingInterval::Monthly, vec![]);
        e.register_plan_agent(&plan.id, "agent-x");
        let inv = e.create_invoice(&plan.id, "buyer-1").unwrap();
        e.pay_invoice(&inv.id).unwrap();

        let payout = e.settle_agent_revenue("agent-x", "2026-03").unwrap();
        assert_eq!(payout.developer_id, "dev-1");
        assert_eq!(payout.amount_cents, 7000); // 70% of 10000
    }

    #[test]
    fn test_settle_unknown_agent() {
        let mut e = engine();
        assert!(e.settle_agent_revenue("unknown", "2026-03").is_none());
    }

    #[test]
    fn test_list_invoices_by_buyer() {
        let mut e = engine();
        let plan = e.create_plan("Pro", 999, BillingInterval::Monthly, vec![]);
        e.create_invoice(&plan.id, "buyer-1").unwrap();
        e.create_invoice(&plan.id, "buyer-2").unwrap();
        e.create_invoice(&plan.id, "buyer-1").unwrap();

        assert_eq!(e.list_invoices(Some("buyer-1")).len(), 2);
        assert_eq!(e.list_invoices(Some("buyer-2")).len(), 1);
        assert_eq!(e.list_invoices(None).len(), 3);
    }

    #[test]
    fn test_list_payouts_by_developer() {
        let mut e = engine();
        e.create_payout("dev-1", "agent-a", 5000, "2026-01");
        e.create_payout("dev-2", "agent-b", 3000, "2026-01");
        e.create_payout("dev-1", "agent-c", 2000, "2026-02");

        assert_eq!(e.list_payouts(Some("dev-1")).len(), 2);
        assert_eq!(e.list_payouts(Some("dev-2")).len(), 1);
        assert_eq!(e.list_payouts(None).len(), 3);
    }

    #[test]
    fn test_revenue_stats() {
        let mut e = engine();
        let plan = e.create_plan("Pro", 10000, BillingInterval::Monthly, vec![]);

        // Create 3 invoices: pay 2, leave 1 pending.
        let i1 = e.create_invoice(&plan.id, "b1").unwrap();
        let i2 = e.create_invoice(&plan.id, "b2").unwrap();
        let _i3 = e.create_invoice(&plan.id, "b3").unwrap();

        e.pay_invoice(&i1.id).unwrap();
        e.pay_invoice(&i2.id).unwrap();

        e.create_payout("dev-1", "agent-x", 7000, "2026-03");

        let stats = e.get_revenue_stats();
        assert_eq!(stats.total_revenue_cents, 20000);
        assert_eq!(stats.total_paid_invoices, 2);
        assert_eq!(stats.total_pending_invoices, 1);
        assert_eq!(stats.developer_share_cents, 14000);
        assert_eq!(stats.platform_share_cents, 6000);
        assert_eq!(stats.total_payouts_cents, 7000);
    }

    #[test]
    fn test_get_invoice() {
        let mut e = engine();
        let plan = e.create_plan("Pro", 999, BillingInterval::Monthly, vec![]);
        let inv = e.create_invoice(&plan.id, "buyer-1").unwrap();
        assert!(e.get_invoice(&inv.id).is_some());
        assert!(e.get_invoice("nope").is_none());
    }
}
