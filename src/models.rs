use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub unit_value: f64,
}

pub struct UserRecord {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Position {
    pub id: i64,
    pub user_id: i64,
    pub asset_id: i64,
    pub quantity: f64,
    pub avg_price: f64,
}

/// Uma compra ou venda registrada, com o nome do ativo.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TransactionRow {
    pub id: i64,
    pub asset_id: i64,
    pub asset_name: String,
    /// `buy` ou `sell`.
    pub kind: String,
    pub quantity: f64,
    pub price: f64,
    /// Preço médio da posição no momento da operação.
    pub cost_basis: f64,
    /// Data no formato `AAAA-MM-DD`.
    pub executed_on: String,
}

/// Posição de uma pessoa usuária já combinada com a cotação atual do ativo.
#[derive(Clone, Debug)]
pub struct PositionRow {
    pub position_id: i64,
    pub asset_id: i64,
    pub asset_name: String,
    pub quantity: f64,
    pub avg_price: f64,
    pub unit_value: f64,
}
