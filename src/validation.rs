//! Regras de entrada compartilhadas pela API e pelas páginas.

use crate::{dates, error::AppError};

fn invalid(message: &str) -> AppError {
    AppError::Validation(message.to_string())
}

pub fn validate_asset_name(name: &str) -> Result<(), AppError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(invalid("O nome do ativo deve ter entre 1 e 60 caracteres"));
    }
    Ok(())
}

pub fn validate_unit_value(unit_value: f64) -> Result<(), AppError> {
    if !unit_value.is_finite() || unit_value <= 0.0 {
        return Err(invalid("O valor unitário deve ser maior que zero"));
    }
    Ok(())
}

pub fn validate_quantity(quantity: f64) -> Result<(), AppError> {
    if !quantity.is_finite() || quantity <= 0.0 {
        return Err(invalid("A quantidade deve ser maior que zero"));
    }
    Ok(())
}

pub fn validate_price(price: f64) -> Result<(), AppError> {
    if !price.is_finite() || price < 0.0 {
        return Err(invalid("O preço não pode ser negativo"));
    }
    Ok(())
}

/// Aceita datas de operação de 1990 até hoje e devolve no formato `AAAA-MM-DD`.
pub fn validate_trade_date(text: &str) -> Result<String, AppError> {
    let Some(date) = dates::parse_iso(text) else {
        return Err(invalid("Informe uma data válida"));
    };
    let earliest = dates::parse_iso("1990-01-01").expect("data fixa válida");
    if date < earliest || date > dates::today() {
        return Err(invalid("A data da operação não pode estar no futuro"));
    }
    Ok(dates::iso(date))
}

pub fn validate_credentials(username: &str, password: &str) -> Result<(), AppError> {
    let length = username.chars().count();
    let allowed = username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    if !(3..=32).contains(&length) || !allowed {
        return Err(invalid(
            "O usuário deve ter de 3 a 32 caracteres (letras, números, _ ou .)",
        ));
    }
    if password.chars().count() < 6 {
        return Err(invalid("A senha deve ter pelo menos 6 caracteres"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_positive_or_non_finite_values() {
        assert!(validate_unit_value(0.0).is_err());
        assert!(validate_unit_value(-1.0).is_err());
        assert!(validate_unit_value(f64::NAN).is_err());
        assert!(validate_quantity(f64::INFINITY).is_err());
        assert!(validate_price(-0.01).is_err());
        assert!(validate_unit_value(0.01).is_ok());
        assert!(validate_price(0.0).is_ok());
    }

    #[test]
    fn validates_asset_names() {
        assert!(validate_asset_name("   ").is_err());
        assert!(validate_asset_name(&"x".repeat(61)).is_err());
        assert!(validate_asset_name("PETR4").is_ok());
    }

    #[test]
    fn validates_credentials() {
        assert!(validate_credentials("ab", "123456").is_err());
        assert!(validate_credentials("ana silva", "123456").is_err());
        assert!(validate_credentials("ana", "12345").is_err());
        assert!(validate_credentials("ana.silva_01", "123456").is_ok());
    }
}
