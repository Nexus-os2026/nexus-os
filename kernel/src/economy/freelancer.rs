//! Freelance Engine — scans for bounties, evaluates profitability, and executes jobs autonomously.

use serde::{Deserialize, Serialize};

/// A supported freelance/bounty platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreelancePlatform {
    pub name: String,
    pub platform_type: PlatformType,
    pub scanner_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlatformType {
    GitHubBounties,
    Gitcoin,
    BugBounty,
    Custom,
}

/// A job opportunity found by scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobOpportunity {
    pub id: String,
    pub platform: String,
    pub title: String,
    pub description: String,
    pub bounty_amount: f64,
    pub currency: String,
    pub difficulty_estimate: f64,
    pub completion_confidence: f64,
    pub estimated_api_cost: f64,
    pub estimated_profit: f64,
    pub deadline: Option<u64>,
    pub url: String,
    pub tags: Vec<String>,
}

/// A completed job with outcome tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedJob {
    pub job: JobOpportunity,
    pub status: JobStatus,
    pub actual_api_cost: f64,
    pub actual_profit: f64,
    pub completion_time_secs: u64,
    pub quality_score: f64,
    pub submission_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Completed,
    Submitted,
    Accepted,
    Rejected,
    Paid,
}

/// HITL mode for job bidding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HitlMode {
    RequireApproval,
    AutoBidBelow(f64),
    FullAuto,
}

/// Revenue and cost tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueTracker {
    pub total_earned: f64,
    pub total_api_cost: f64,
    pub total_profit: f64,
    pub jobs_completed: u32,
    pub jobs_accepted: u32,
    pub jobs_rejected: u32,
    pub success_rate: f64,
    pub avg_profit_per_job: f64,
}

impl Default for RevenueTracker {
    fn default() -> Self {
        Self {
            total_earned: 0.0,
            total_api_cost: 0.0,
            total_profit: 0.0,
            jobs_completed: 0,
            jobs_accepted: 0,
            jobs_rejected: 0,
            success_rate: 0.0,
            avg_profit_per_job: 0.0,
        }
    }
}

impl RevenueTracker {
    /// Record a completed job.
    pub fn record_job(&mut self, job: &CompletedJob) {
        self.jobs_completed += 1;
        self.total_api_cost += job.actual_api_cost;

        match job.status {
            JobStatus::Accepted | JobStatus::Paid => {
                self.jobs_accepted += 1;
                self.total_earned += job.job.bounty_amount;
                self.total_profit += job.actual_profit;
            }
            JobStatus::Rejected => {
                self.jobs_rejected += 1;
                // Still paid API cost but earned nothing
                self.total_profit -= job.actual_api_cost;
            }
            _ => {}
        }

        if self.jobs_completed > 0 {
            self.success_rate = self.jobs_accepted as f64 / self.jobs_completed as f64;
            self.avg_profit_per_job = self.total_profit / self.jobs_completed as f64;
        }
    }

    /// Is the engine profitable overall?
    pub fn is_profitable(&self) -> bool {
        self.total_profit > 0.0
    }
}

/// The freelance engine that scans, evaluates, and executes jobs.
#[derive(Debug, Clone)]
pub struct FreelanceEngine {
    pub scanning: bool,
    pub platforms: Vec<FreelancePlatform>,
    pub opportunities: Vec<JobOpportunity>,
    pub completed_jobs: Vec<CompletedJob>,
    pub revenue: RevenueTracker,
    pub hitl_mode: HitlMode,
    pub min_profit_margin: f64,
    pub max_concurrent_jobs: u32,
    pub active_jobs: u32,
}

impl Default for FreelanceEngine {
    fn default() -> Self {
        Self {
            scanning: false,
            platforms: vec![
                FreelancePlatform {
                    name: "GitHub Bounties".into(),
                    platform_type: PlatformType::GitHubBounties,
                    scanner_url: "https://api.github.com".into(),
                    enabled: true,
                },
                FreelancePlatform {
                    name: "Gitcoin".into(),
                    platform_type: PlatformType::Gitcoin,
                    scanner_url: "https://gitcoin.co/api".into(),
                    enabled: true,
                },
                FreelancePlatform {
                    name: "Bug Bounty".into(),
                    platform_type: PlatformType::BugBounty,
                    scanner_url: "https://hackerone.com/api".into(),
                    enabled: false,
                },
            ],
            opportunities: Vec::new(),
            completed_jobs: Vec::new(),
            revenue: RevenueTracker::default(),
            hitl_mode: HitlMode::RequireApproval,
            min_profit_margin: 1.5,
            max_concurrent_jobs: 3,
            active_jobs: 0,
        }
    }
}

impl FreelanceEngine {
    /// Start scanning for opportunities.
    pub fn start_scanning(&mut self) {
        self.scanning = true;
    }

    /// Stop scanning.
    pub fn stop_scanning(&mut self) {
        self.scanning = false;
    }

    /// Evaluate whether a job is profitable enough to take.
    pub fn evaluate_opportunity(&self, job: &JobOpportunity) -> JobEvaluation {
        let profit_ratio = if job.estimated_api_cost > 0.0 {
            job.bounty_amount / job.estimated_api_cost
        } else {
            f64::MAX
        };

        let profitable = profit_ratio >= self.min_profit_margin;
        let capacity = self.active_jobs < self.max_concurrent_jobs;
        let confidence_ok = job.completion_confidence >= 0.7;

        let should_bid = profitable && capacity && confidence_ok;

        let needs_approval = match &self.hitl_mode {
            HitlMode::RequireApproval => true,
            HitlMode::AutoBidBelow(threshold) => job.bounty_amount >= *threshold,
            HitlMode::FullAuto => false,
        };

        JobEvaluation {
            job_id: job.id.clone(),
            profit_ratio,
            profitable,
            has_capacity: capacity,
            confidence_ok,
            should_bid,
            needs_approval,
        }
    }

    /// Filter opportunities to only profitable ones.
    pub fn filter_profitable(&self) -> Vec<&JobOpportunity> {
        self.opportunities
            .iter()
            .filter(|job| {
                let eval = self.evaluate_opportunity(job);
                eval.should_bid
            })
            .collect()
    }

    /// Record a completed job and update revenue tracking.
    pub fn record_completion(&mut self, job: CompletedJob) {
        if self.active_jobs > 0 {
            self.active_jobs -= 1;
        }
        self.revenue.record_job(&job);
        self.completed_jobs.push(job);
    }

    /// Get a summary of the engine state for UI display.
    pub fn get_status(&self) -> FreelanceStatus {
        FreelanceStatus {
            scanning: self.scanning,
            platforms_enabled: self.platforms.iter().filter(|p| p.enabled).count(),
            opportunities_found: self.opportunities.len(),
            active_jobs: self.active_jobs,
            total_earned: self.revenue.total_earned,
            total_api_cost: self.revenue.total_api_cost,
            total_profit: self.revenue.total_profit,
            jobs_completed: self.revenue.jobs_completed,
            success_rate: self.revenue.success_rate,
            is_profitable: self.revenue.is_profitable(),
            hitl_mode: self.hitl_mode.clone(),
        }
    }

    /// Build LLM prompt for evaluating a job's difficulty and completion confidence.
    pub fn build_evaluation_prompt(&self, job: &JobOpportunity) -> (String, String) {
        let system = "You are a job evaluation engine. Given a freelance task description, \
                       estimate the difficulty, required capabilities, API cost, and completion confidence."
            .to_string();
        let user = format!(
            "Evaluate this freelance job:\n\
             Title: {}\n\
             Description: {}\n\
             Bounty: {} {}\n\
             Tags: {}\n\n\
             Estimate:\n\
             1. difficulty (0-1)\n\
             2. completion_confidence (0-1) — can our AI agents handle this?\n\
             3. estimated_api_cost in USD\n\
             4. estimated_hours\n\
             5. required_capabilities (list)\n\
             Return JSON.",
            job.title,
            job.description,
            job.bounty_amount,
            job.currency,
            job.tags.join(", ")
        );
        (system, user)
    }
}

/// Result of evaluating a job opportunity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEvaluation {
    pub job_id: String,
    pub profit_ratio: f64,
    pub profitable: bool,
    pub has_capacity: bool,
    pub confidence_ok: bool,
    pub should_bid: bool,
    pub needs_approval: bool,
}

/// Status summary of the freelance engine for UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreelanceStatus {
    pub scanning: bool,
    pub platforms_enabled: usize,
    pub opportunities_found: usize,
    pub active_jobs: u32,
    pub total_earned: f64,
    pub total_api_cost: f64,
    pub total_profit: f64,
    pub jobs_completed: u32,
    pub success_rate: f64,
    pub is_profitable: bool,
    pub hitl_mode: HitlMode,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_job(bounty: f64, api_cost: f64, confidence: f64) -> JobOpportunity {
        JobOpportunity {
            id: "job-1".into(),
            platform: "github".into(),
            title: "Fix bug in login".into(),
            description: "The login form crashes on empty input".into(),
            bounty_amount: bounty,
            currency: "USD".into(),
            difficulty_estimate: 0.3,
            completion_confidence: confidence,
            estimated_api_cost: api_cost,
            estimated_profit: bounty - api_cost,
            deadline: Some(1700000000),
            url: "https://github.com/example/repo/issues/1".into(),
            tags: vec!["bug".into(), "authentication".into()],
        }
    }

    #[test]
    fn test_freelance_engine_default() {
        let engine = FreelanceEngine::default();
        assert!(!engine.scanning);
        assert_eq!(engine.platforms.len(), 3);
        assert!((engine.min_profit_margin - 1.5).abs() < f64::EPSILON);
        assert_eq!(engine.hitl_mode, HitlMode::RequireApproval);
    }

    #[test]
    fn test_evaluate_profitable_job() {
        let engine = FreelanceEngine::default();
        // bounty $100, cost $20 → ratio 5.0 > 1.5 ✓
        let job = test_job(100.0, 20.0, 0.9);
        let eval = engine.evaluate_opportunity(&job);
        assert!(eval.profitable);
        assert!(eval.confidence_ok);
        assert!(eval.should_bid);
        assert!(eval.needs_approval); // RequireApproval mode
    }

    #[test]
    fn test_evaluate_unprofitable_job() {
        let engine = FreelanceEngine::default();
        // bounty $10, cost $20 → ratio 0.5 < 1.5 ✗
        let job = test_job(10.0, 20.0, 0.9);
        let eval = engine.evaluate_opportunity(&job);
        assert!(!eval.profitable);
        assert!(!eval.should_bid);
    }

    #[test]
    fn test_evaluate_low_confidence_job() {
        let engine = FreelanceEngine::default();
        // Profitable but low confidence
        let job = test_job(100.0, 20.0, 0.3);
        let eval = engine.evaluate_opportunity(&job);
        assert!(eval.profitable);
        assert!(!eval.confidence_ok);
        assert!(!eval.should_bid);
    }

    #[test]
    fn test_evaluate_at_capacity() {
        let mut engine = FreelanceEngine::default();
        engine.active_jobs = engine.max_concurrent_jobs;
        let job = test_job(100.0, 20.0, 0.9);
        let eval = engine.evaluate_opportunity(&job);
        assert!(!eval.has_capacity);
        assert!(!eval.should_bid);
    }

    #[test]
    fn test_hitl_auto_bid_below() {
        let engine = FreelanceEngine {
            hitl_mode: HitlMode::AutoBidBelow(50.0),
            ..Default::default()
        };

        let cheap_job = test_job(30.0, 5.0, 0.9);
        let eval = engine.evaluate_opportunity(&cheap_job);
        assert!(!eval.needs_approval); // $30 < $50 threshold

        let expensive_job = test_job(100.0, 20.0, 0.9);
        let eval = engine.evaluate_opportunity(&expensive_job);
        assert!(eval.needs_approval); // $100 >= $50 threshold
    }

    #[test]
    fn test_hitl_full_auto() {
        let engine = FreelanceEngine {
            hitl_mode: HitlMode::FullAuto,
            ..Default::default()
        };
        let job = test_job(1000.0, 100.0, 0.9);
        let eval = engine.evaluate_opportunity(&job);
        assert!(!eval.needs_approval);
    }

    #[test]
    fn test_filter_profitable() {
        let engine = FreelanceEngine {
            opportunities: vec![
                test_job(100.0, 20.0, 0.9), // profitable ✓
                test_job(10.0, 20.0, 0.9),  // not profitable ✗
                test_job(100.0, 20.0, 0.3), // low confidence ✗
            ],
            ..Default::default()
        };
        let profitable = engine.filter_profitable();
        assert_eq!(profitable.len(), 1);
        assert_eq!(profitable[0].bounty_amount, 100.0);
    }

    #[test]
    fn test_revenue_tracker_record() {
        let mut tracker = RevenueTracker::default();
        let completed = CompletedJob {
            job: test_job(100.0, 20.0, 0.9),
            status: JobStatus::Paid,
            actual_api_cost: 18.0,
            actual_profit: 82.0,
            completion_time_secs: 3600,
            quality_score: 9.0,
            submission_url: Some("https://github.com/example/pr/1".into()),
        };
        tracker.record_job(&completed);
        assert_eq!(tracker.jobs_completed, 1);
        assert_eq!(tracker.jobs_accepted, 1);
        assert!((tracker.total_earned - 100.0).abs() < f64::EPSILON);
        assert!((tracker.total_api_cost - 18.0).abs() < f64::EPSILON);
        assert!(tracker.is_profitable());
        assert!((tracker.success_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_revenue_tracker_rejected() {
        let mut tracker = RevenueTracker::default();
        let rejected = CompletedJob {
            job: test_job(100.0, 20.0, 0.9),
            status: JobStatus::Rejected,
            actual_api_cost: 25.0,
            actual_profit: -25.0,
            completion_time_secs: 3600,
            quality_score: 4.0,
            submission_url: None,
        };
        tracker.record_job(&rejected);
        assert_eq!(tracker.jobs_rejected, 1);
        assert!(!tracker.is_profitable());
    }

    #[test]
    fn test_record_completion() {
        let mut engine = FreelanceEngine {
            active_jobs: 1,
            ..Default::default()
        };
        let completed = CompletedJob {
            job: test_job(100.0, 20.0, 0.9),
            status: JobStatus::Paid,
            actual_api_cost: 18.0,
            actual_profit: 82.0,
            completion_time_secs: 3600,
            quality_score: 9.0,
            submission_url: None,
        };
        engine.record_completion(completed);
        assert_eq!(engine.active_jobs, 0);
        assert_eq!(engine.completed_jobs.len(), 1);
        assert!(engine.revenue.is_profitable());
    }

    #[test]
    fn test_get_status() {
        let engine = FreelanceEngine::default();
        let status = engine.get_status();
        assert!(!status.scanning);
        assert_eq!(status.platforms_enabled, 2); // github + gitcoin enabled
        assert!(!status.is_profitable);
    }

    #[test]
    fn test_start_stop_scanning() {
        let mut engine = FreelanceEngine::default();
        engine.start_scanning();
        assert!(engine.scanning);
        engine.stop_scanning();
        assert!(!engine.scanning);
    }

    #[test]
    fn test_build_evaluation_prompt() {
        let engine = FreelanceEngine::default();
        let job = test_job(100.0, 20.0, 0.9);
        let (system, user) = engine.build_evaluation_prompt(&job);
        assert!(system.contains("evaluation"));
        assert!(user.contains("Fix bug in login"));
        assert!(user.contains("100"));
    }
}
