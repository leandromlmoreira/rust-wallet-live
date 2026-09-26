//! Formatação de números no padrão brasileiro para as páginas.

fn group_thousands(integer: u64) -> String {
    let digits = integer.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(digit);
    }
    grouped
}

/// `1234.5` -> `R$ 1.234,50`
pub fn brl(value: f64) -> String {
    let cents = (value.abs() * 100.0).round() as u64;
    let sign = if value < 0.0 && cents > 0 { "-" } else { "" };
    format!(
        "{sign}R$ {},{:02}",
        group_thousands(cents / 100),
        cents % 100
    )
}

/// Rótulo curto para eixos: `R$ 0`, `R$ 750`, `R$ 2,5 mil`, `R$ 40 mil`, `R$ 1,2 mi`.
pub fn brl_compact(value: f64) -> String {
    let (scaled, suffix) = if value.abs() >= 1_000_000.0 {
        (value / 1_000_000.0, " mi")
    } else if value.abs() >= 1_000.0 {
        (value / 1_000.0, " mil")
    } else {
        (value, "")
    };
    let text = if (scaled - scaled.round()).abs() < 0.05 {
        format!("{}", scaled.round() as i64)
    } else {
        format!("{scaled:.1}").replace('.', ",")
    };
    format!("R$ {text}{suffix}")
}

/// `32.894` -> `32,9%`
pub fn percent(value: f64) -> String {
    format!("{value:.1}%").replace('.', ",")
}

/// `12.345` -> `+12,35%`
pub fn signed_percent(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let sign = if rounded > 0.0 {
        "+"
    } else if rounded < 0.0 {
        "-"
    } else {
        ""
    };
    format!("{sign}{:.2}%", rounded.abs()).replace('.', ",")
}

/// `0.5` -> `0,5`, `3.0` -> `3` (até 8 casas, sem zeros à direita).
pub fn quantity(value: f64) -> String {
    let text = format!("{value:.8}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    let (integer, fraction) = text.split_once('.').unwrap_or((text, ""));
    let integer = group_thousands(integer.parse().unwrap_or(0));
    if fraction.is_empty() {
        integer
    } else {
        format!("{integer},{fraction}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_currency() {
        assert_eq!(brl(0.0), "R$ 0,00");
        assert_eq!(brl(1234.5), "R$ 1.234,50");
        assert_eq!(brl(1_000_000.0), "R$ 1.000.000,00");
        assert_eq!(brl(-42.199), "-R$ 42,20");
        assert_eq!(brl(-0.001), "R$ 0,00");
    }

    #[test]
    fn formats_compact_axis_labels() {
        assert_eq!(brl_compact(0.0), "R$ 0");
        assert_eq!(brl_compact(750.0), "R$ 750");
        assert_eq!(brl_compact(2_500.0), "R$ 2,5 mil");
        assert_eq!(brl_compact(40_000.0), "R$ 40 mil");
        assert_eq!(brl_compact(1_200_000.0), "R$ 1,2 mi");
        assert_eq!(percent(32.894), "32,9%");
    }

    #[test]
    fn formats_percent() {
        assert_eq!(signed_percent(12.345), "+12,35%");
        assert_eq!(signed_percent(-3.2), "-3,20%");
        assert_eq!(signed_percent(0.0), "0,00%");
    }

    #[test]
    fn formats_quantity() {
        assert_eq!(quantity(3.0), "3");
        assert_eq!(quantity(0.5), "0,5");
        assert_eq!(quantity(1500.25), "1.500,25");
        assert_eq!(quantity(0.00012345), "0,00012345");
    }
}
