use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetSignal {
    Normal,
    Alert50,
    Pause90,
    Stop100,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisionCostSnapshot {
    pub budget: u64,
    pub consumed: u64,
    pub remaining: u64,
    pub signal: BudgetSignal,
}

#[derive(Debug, Clone)]
pub struct VisionCostController {
    budget: u64,
    consumed: u64,
    alert_issued: bool,
    pause_issued: bool,
}

impl VisionCostController {
    pub fn new(budget: u64) -> Self {
        Self {
            budget,
            consumed: 0,
            alert_issued: false,
            pause_issued: false,
        }
    }

    pub fn consume(&mut self, amount: u64) -> VisionCostSnapshot {
        self.consumed = self.consumed.saturating_add(amount).min(self.budget);

        let signal = if self.budget == 0 || self.consumed >= self.budget {
            BudgetSignal::Stop100
        } else {
            let percentage = self.consumed.saturating_mul(100) / self.budget;
            if percentage >= 90 {
                if self.pause_issued {
                    BudgetSignal::Normal
                } else {
                    self.pause_issued = true;
                    BudgetSignal::Pause90
                }
            } else if percentage >= 50 {
                if self.alert_issued {
                    BudgetSignal::Normal
                } else {
                    self.alert_issued = true;
                    BudgetSignal::Alert50
                }
            } else {
                BudgetSignal::Normal
            }
        };

        VisionCostSnapshot {
            budget: self.budget,
            consumed: self.consumed,
            remaining: self.budget.saturating_sub(self.consumed),
            signal,
        }
    }

    pub fn consumed(&self) -> u64 {
        self.consumed
    }

    pub fn remaining(&self) -> u64 {
        self.budget.saturating_sub(self.consumed)
    }

    pub fn budget(&self) -> u64 {
        self.budget
    }
}

#[cfg(test)]
mod tests {
    use super::{BudgetSignal, VisionCostController};

    #[test]
    fn test_cost_threshold_signals() {
        let mut controller = VisionCostController::new(1_000);

        let first = controller.consume(500);
        assert_eq!(first.signal, BudgetSignal::Alert50);

        let second = controller.consume(400);
        assert_eq!(second.signal, BudgetSignal::Pause90);

        let third = controller.consume(100);
        assert_eq!(third.signal, BudgetSignal::Stop100);
        assert_eq!(third.remaining, 0);
    }
}
