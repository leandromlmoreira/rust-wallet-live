//! Geometria dos gráficos, calculada no servidor e desenhada em SVG pelos templates.
//!
//! Regras de visualização aplicadas: um único eixo, linhas de 2px, marcadores com anel
//! da cor da superfície, cores categóricas em ordem fixa por ativo (nunca pelo ranking)
//! e texto sempre com as cores de texto, nunca com a cor da série.

use time::Date;

use crate::{
    dates, format,
    portfolio::{HoldingSummary, TimelinePoint},
};

/// Paleta categórica validada (claro e escuro), em ordem fixa. Os valores ficam em CSS.
pub const SERIES: [&str; 8] = [
    "var(--series-1)",
    "var(--series-2)",
    "var(--series-3)",
    "var(--series-4)",
    "var(--series-5)",
    "var(--series-6)",
    "var(--series-7)",
    "var(--series-8)",
];

/// Cor neutra para o que passa das 8 cores ("Outros").
pub const OTHER: &str = "var(--series-other)";

pub fn series_color(slot: usize) -> &'static str {
    SERIES.get(slot).copied().unwrap_or(OTHER)
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// Topo "redondo" do eixo e o passo entre divisões (3 a 5 divisões),
/// escolhendo o menor topo que cabe o valor máximo. Em empate, prefere 4 divisões.
fn nice_scale(max: f64) -> (f64, f64) {
    if max <= 0.0 {
        return (4.0, 1.0);
    }
    let magnitude = 10f64.powf((max / 5.0).log10().floor());
    let mut best = (f64::INFINITY, 1.0);
    for divisions in [4.0, 5.0, 3.0] {
        for factor in [1.0, 2.0, 2.5, 5.0, 10.0, 20.0] {
            let step = factor * magnitude;
            let top = step * divisions;
            if top >= max && top < best.0 {
                best = (top, step);
            }
        }
    }
    best
}

pub struct Tick {
    pub pos: f64,
    pub label: String,
}

pub struct ChartPoint {
    pub x: f64,
    pub y: f64,
    pub tip: String,
}

pub struct LineChart {
    pub width: f64,
    pub height: f64,
    pub plot_left: f64,
    pub plot_right: f64,
    pub baseline: f64,
    pub line_path: String,
    pub area_path: String,
    pub y_ticks: Vec<Tick>,
    pub x_ticks: Vec<Tick>,
    pub points: Vec<ChartPoint>,
    pub end_x: f64,
    pub end_y: f64,
    pub end_label: String,
}

const WIDTH: f64 = 720.0;
const HEIGHT: f64 = 260.0;
const LEFT: f64 = 76.0;
const RIGHT: f64 = 24.0;
const TOP: f64 = 28.0;
const BOTTOM: f64 = 32.0;

/// Linha em degraus do capital investido. Precisa de pelo menos duas datas.
/// O último valor segue até `today`, porque o capital continua aplicado.
pub fn invested_line(timeline: &[TimelinePoint], today: Date) -> Option<LineChart> {
    let parsed: Vec<(Date, f64)> = timeline
        .iter()
        .filter_map(|point| Some((dates::parse_iso(&point.date)?, point.invested)))
        .collect();
    if parsed.len() < 2 {
        return None;
    }

    let start = parsed.first()?.0;
    let end = parsed.last()?.0.max(today);
    let span = f64::from((end.to_julian_day() - start.to_julian_day()).max(1));
    let max_value = parsed.iter().map(|(_, value)| *value).fold(0.0, f64::max);
    let (y_max, y_step) = nice_scale(max_value);

    let plot_width = WIDTH - LEFT - RIGHT;
    let plot_height = HEIGHT - TOP - BOTTOM;
    let baseline = TOP + plot_height;
    let x_of = |date: Date| {
        round1(LEFT + f64::from(date.to_julian_day() - start.to_julian_day()) / span * plot_width)
    };
    let y_of = |value: f64| round1(baseline - value.max(0.0) / y_max * plot_height);

    let mut line_path = String::new();
    let mut points = Vec::with_capacity(parsed.len());
    for (index, (date, value)) in parsed.iter().enumerate() {
        let (x, y) = (x_of(*date), y_of(*value));
        if index == 0 {
            line_path.push_str(&format!("M{x} {y}"));
        } else {
            line_path.push_str(&format!(" H{x} V{y}"));
        }
        points.push(ChartPoint {
            x,
            y,
            tip: format!("{}: {} investido", dates::br(*date), format::brl(*value)),
        });
    }

    let last_value = parsed.last()?.1;
    let end_x = x_of(end);
    let end_y = y_of(last_value);
    line_path.push_str(&format!(" H{end_x}"));
    let first_x = x_of(start);
    let area_path = format!("{line_path} V{baseline} H{first_x} Z");

    let divisions = (y_max / y_step).round() as i32;
    let y_ticks = (0..=divisions)
        .map(|index| {
            let value = y_step * f64::from(index);
            Tick {
                pos: y_of(value),
                label: format::brl_compact(value),
            }
        })
        .collect();

    let mut x_ticks: Vec<Tick> = Vec::new();
    for index in 0..4 {
        let offset = (span * f64::from(index) / 3.0).round() as i32;
        let date = Date::from_julian_day(start.to_julian_day() + offset).unwrap_or(start);
        let label = dates::br_short(date);
        if x_ticks.last().is_some_and(|tick| tick.label == label) {
            continue;
        }
        x_ticks.push(Tick {
            pos: x_of(date),
            label,
        });
    }

    Some(LineChart {
        width: WIDTH,
        height: HEIGHT,
        plot_left: LEFT,
        plot_right: WIDTH - RIGHT,
        baseline,
        line_path,
        area_path,
        y_ticks,
        x_ticks,
        points,
        end_x,
        end_y,
        end_label: format::brl(last_value),
    })
}

pub struct DonutSlice {
    pub path: String,
    pub color: &'static str,
    pub tip: String,
}

const CX: f64 = 100.0;
const CY: f64 = 100.0;
const OUTER: f64 = 92.0;
const INNER: f64 = 62.0;

fn polar(radius: f64, angle: f64) -> (f64, f64) {
    (
        round1(CX + radius * angle.cos()),
        round1(CY + radius * angle.sin()),
    )
}

fn annular_sector(start: f64, end: f64) -> String {
    let large = if end - start > std::f64::consts::PI {
        1
    } else {
        0
    };
    let (ox1, oy1) = polar(OUTER, start);
    let (ox2, oy2) = polar(OUTER, end);
    let (ix2, iy2) = polar(INNER, end);
    let (ix1, iy1) = polar(INNER, start);
    format!(
        "M{ox1} {oy1} A{OUTER} {OUTER} 0 {large} 1 {ox2} {oy2} L{ix2} {iy2} A{INNER} {INNER} 0 {large} 0 {ix1} {iy1} Z"
    )
}

/// Rosca de alocação, do maior para o menor ativo, começando no topo.
pub fn allocation_donut(holdings: &[HoldingSummary]) -> Vec<DonutSlice> {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    let total: f64 = holdings.iter().map(|h| h.market_value).sum();
    if total <= 0.0 {
        return Vec::new();
    }

    let mut angle = -FRAC_PI_2;
    holdings
        .iter()
        .filter(|h| h.market_value > 0.0)
        .map(|holding| {
            let sweep = holding.market_value / total * TAU;
            // Uma fatia de 100% vira duas metades, porque um arco SVG não fecha o círculo.
            let path = if sweep >= TAU - 1e-9 {
                format!(
                    "{} {}",
                    annular_sector(angle, angle + PI),
                    annular_sector(angle + PI, angle + TAU)
                )
            } else {
                annular_sector(angle, angle + sweep)
            };
            angle += sweep;
            DonutSlice {
                path,
                color: series_color(holding.color_slot),
                tip: format!(
                    "{}: {} ({})",
                    holding.asset_name,
                    format::brl(holding.market_value),
                    format::percent(holding.allocation_pct)
                ),
            }
        })
        .collect()
}

pub struct ResultBar {
    pub name: String,
    pub color: &'static str,
    pub value: String,
    pub pct: String,
    pub positive: bool,
    /// Comprimento da barra, em % da metade do gráfico.
    pub width: String,
}

/// Barras divergentes do resultado por ativo, do maior ganho à maior perda.
pub fn result_bars(holdings: &[HoldingSummary]) -> Vec<ResultBar> {
    let max = holdings.iter().map(|h| h.pnl.abs()).fold(0.0, f64::max);

    let mut rows: Vec<&HoldingSummary> = holdings.iter().collect();
    rows.sort_by(|a, b| b.pnl.total_cmp(&a.pnl));
    rows.into_iter()
        .map(|holding| ResultBar {
            name: holding.asset_name.clone(),
            color: series_color(holding.color_slot),
            value: format::brl(holding.pnl),
            pct: format::signed_percent(holding.pnl_pct),
            positive: holding.pnl >= 0.0,
            width: format!(
                "{:.2}",
                if max > 0.0 {
                    holding.pnl.abs() / max * 100.0
                } else {
                    0.0
                }
            ),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(date: &str, invested: f64) -> TimelinePoint {
        TimelinePoint {
            date: date.to_string(),
            invested,
        }
    }

    fn holding(name: &str, slot: usize, market_value: f64, pnl: f64) -> HoldingSummary {
        HoldingSummary {
            position_id: slot as i64,
            asset_id: slot as i64,
            asset_name: name.to_string(),
            quantity: 1.0,
            avg_price: 1.0,
            unit_value: 1.0,
            invested: market_value - pnl,
            market_value,
            pnl,
            pnl_pct: 0.0,
            allocation_pct: 0.0,
            color_slot: slot,
        }
    }

    #[test]
    fn nice_scale_rounds_up_to_clean_steps() {
        assert_eq!(nice_scale(23_348.0), (25_000.0, 5_000.0));
        assert_eq!(nice_scale(26_411.0), (30_000.0, 10_000.0));
        assert_eq!(nice_scale(7_600.0), (8_000.0, 2_000.0));
        assert_eq!(nice_scale(9_000.0), (10_000.0, 2_500.0));
        assert_eq!(nice_scale(420.0), (500.0, 100.0));
        assert_eq!(nice_scale(0.0), (4.0, 1.0));
    }

    #[test]
    fn line_needs_two_dates_and_extends_to_today() {
        let today = dates::parse_iso("2026-09-26").unwrap();
        assert!(invested_line(&[point("2026-01-01", 10.0)], today).is_none());

        let chart = invested_line(
            &[point("2026-01-01", 1000.0), point("2026-05-01", 2500.0)],
            today,
        )
        .unwrap();
        assert_eq!(chart.points.len(), 2);
        assert_eq!(chart.end_x, chart.plot_right);
        assert!(chart.points[1].x < chart.end_x);
        assert!(chart.line_path.starts_with('M'));
        assert!(chart.area_path.ends_with('Z'));
        assert!((4..=6).contains(&chart.y_ticks.len()));
        assert_eq!(chart.end_label, "R$ 2.500,00");
    }

    #[test]
    fn donut_uses_fixed_colors_and_closes_a_full_circle() {
        let slices = allocation_donut(&[holding("A", 2, 100.0, 0.0)]);
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].color, "var(--series-3)");
        assert_eq!(slices[0].path.matches('M').count(), 2);

        assert!(allocation_donut(&[]).is_empty());
        assert_eq!(series_color(9), OTHER);
    }

    #[test]
    fn result_bars_are_sorted_and_scaled_to_the_largest_move() {
        let bars = result_bars(&[
            holding("Perde", 0, 50.0, -50.0),
            holding("Ganha", 1, 300.0, 100.0),
        ]);
        assert_eq!(bars[0].name, "Ganha");
        assert_eq!(bars[0].width, "100.00");
        assert!(!bars[1].positive);
        assert_eq!(bars[1].width, "50.00");
    }
}
