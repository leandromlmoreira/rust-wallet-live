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
