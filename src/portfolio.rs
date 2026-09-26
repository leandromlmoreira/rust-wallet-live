//! Cálculos da carteira: valor investido, valor de mercado, lucro e alocação.

use serde::Serialize;

use crate::models::PositionRow;

#[derive(Serialize, Debug, PartialEq)]
pub struct HoldingSummary {
    pub position_id: i64,
    pub asset_id: i64,
    pub asset_name: String,
    pub quantity: f64,
    pub avg_price: f64,
    pub unit_value: f64,
    pub invested: f64,
    pub market_value: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub allocation_pct: f64,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct PortfolioSummary {
    pub holdings: Vec<HoldingSummary>,
    pub total_invested: f64,
    pub market_value: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
}

fn percent_of(part: f64, whole: f64) -> f64 {
    if whole > 0.0 {
        part / whole * 100.0
    } else {
        0.0
    }
}

/// Resume as posições, ordenando da maior para a menor fatia da carteira.
pub fn summarize(rows: Vec<PositionRow>) -> PortfolioSummary {
    let market_value: f64 = rows.iter().map(|row| row.quantity * row.unit_value).sum();
    let total_invested: f64 = rows.iter().map(|row| row.quantity * row.avg_price).sum();

    let mut holdings: Vec<HoldingSummary> = rows
        .into_iter()
        .map(|row| {
            let invested = row.quantity * row.avg_price;
            let holding_value = row.quantity * row.unit_value;
            HoldingSummary {
                position_id: row.position_id,
                asset_id: row.asset_id,
                asset_name: row.asset_name,
                quantity: row.quantity,
                avg_price: row.avg_price,
                unit_value: row.unit_value,
                invested,
                market_value: holding_value,
                pnl: holding_value - invested,
                pnl_pct: percent_of(holding_value - invested, invested),
                allocation_pct: percent_of(holding_value, market_value),
            }
        })
        .collect();

    holdings.sort_by(|a, b| b.market_value.total_cmp(&a.market_value));

    PortfolioSummary {
        holdings,
        total_invested,
        market_value,
        pnl: market_value - total_invested,
        pnl_pct: percent_of(market_value - total_invested, total_invested),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, name: &str, quantity: f64, avg_price: f64, unit_value: f64) -> PositionRow {
        PositionRow {
            position_id: id,
            asset_id: id,
            asset_name: name.to_string(),
            quantity,
            avg_price,
            unit_value,
        }
    }

    #[test]
    fn empty_portfolio_has_zeroed_totals() {
        let summary = summarize(vec![]);
        assert!(summary.holdings.is_empty());
        assert_eq!(summary.market_value, 0.0);
        assert_eq!(summary.pnl_pct, 0.0);
    }

    #[test]
    fn computes_totals_pnl_and_allocation() {
        let summary = summarize(vec![
            // investido 3000, vale 4000
            row(1, "PETR4", 100.0, 30.0, 40.0),
            // investido 4000, vale 3000
            row(2, "Bitcoin", 0.01, 400_000.0, 300_000.0),
        ]);

        assert_eq!(summary.total_invested, 7000.0);
        assert_eq!(summary.market_value, 7000.0);
        assert_eq!(summary.pnl, 0.0);

        let petr = &summary.holdings[0];
        assert_eq!(petr.asset_name, "PETR4");
        assert_eq!(petr.pnl, 1000.0);
        assert!((petr.pnl_pct - 33.333).abs() < 0.01);
        assert!((petr.allocation_pct - 57.142).abs() < 0.01);

        let btc = &summary.holdings[1];
        assert!((btc.pnl + 1000.0).abs() < 1e-6);
        assert!((btc.pnl_pct + 25.0).abs() < 1e-6);
    }

    #[test]
    fn allocation_adds_up_to_one_hundred_percent() {
        let summary = summarize(vec![
            row(1, "A", 3.0, 1.0, 10.0),
            row(2, "B", 1.0, 1.0, 5.0),
            row(3, "C", 7.0, 1.0, 2.5),
        ]);
        let total: f64 = summary.holdings.iter().map(|h| h.allocation_pct).sum();
        assert!((total - 100.0).abs() < 1e-9);
    }

    #[test]
    fn free_positions_do_not_divide_by_zero() {
        let summary = summarize(vec![row(1, "Airdrop", 5.0, 0.0, 2.0)]);
        assert_eq!(summary.total_invested, 0.0);
        assert_eq!(summary.pnl, 10.0);
        assert_eq!(summary.pnl_pct, 0.0);
    }
}
