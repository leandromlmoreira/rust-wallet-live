use std::convert::Infallible;

use axum::extract::FromRequestParts;
use sqlx::PgPool;

use crate::{
    app::AppState,
    models::{Asset, Position, PositionRow, UserRecord},
};

/// Tolerância para comparar quantidades fracionadas (ex.: 0.1 + 0.2).
const QUANTITY_EPSILON: f64 = 1e-9;

#[derive(Debug, PartialEq)]
pub enum SellOutcome {
    /// A posição continua aberta com a quantidade restante.
    Reduced(f64),
    /// Toda a quantidade foi vendida e a posição foi encerrada.
    Closed,
    NotFound,
    InsufficientQuantity,
}

pub struct Repository {
    db: PgPool,
}

impl Repository {
    pub async fn list_assets(&self) -> sqlx::Result<Vec<Asset>> {
        sqlx::query_as!(
            Asset,
            "SELECT id, name, unit_value
             FROM assets
             ORDER BY id;"
        )
        .fetch_all(&self.db)
        .await
    }

    pub async fn create_asset(&self, name: String, unit_value: f64) -> sqlx::Result<Asset> {
        sqlx::query_as!(
            Asset,
            "INSERT INTO assets (name, unit_value)
             VALUES ($1, $2)
             RETURNING id, name, unit_value;",
            name,
            unit_value
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn update_asset(
        &self,
        asset_id: i64,
        name: Option<String>,
        unit_value: Option<f64>,
    ) -> sqlx::Result<Option<Asset>> {
        sqlx::query_as!(
            Asset,
            "UPDATE assets
             SET name=COALESCE($2, name),
                 unit_value=COALESCE($3, unit_value)
             WHERE id=$1
             RETURNING id, name, unit_value;",
            asset_id,
            name,
            unit_value
        )
        .fetch_optional(&self.db)
        .await
    }

    pub async fn add_user(&self, username: &str, password_hash: &str) -> sqlx::Result<UserRecord> {
        sqlx::query_as!(
            UserRecord,
            "INSERT INTO users (username, password_hash)
             VALUES ($1, $2)
             RETURNING id, username, password_hash;",
            username,
            password_hash,
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn get_user_by_name(&self, username: &str) -> sqlx::Result<Option<UserRecord>> {
        sqlx::query_as!(
            UserRecord,
            "SELECT id, username, password_hash
             FROM users
             WHERE username = $1;",
            username
        )
        .fetch_optional(&self.db)
        .await
    }

    pub async fn list_positions(&self, user_id: i64) -> sqlx::Result<Vec<PositionRow>> {
        sqlx::query_as!(
            PositionRow,
            "SELECT p.id AS position_id, a.id AS asset_id, a.name AS asset_name,
                    p.quantity, p.avg_price, a.unit_value
             FROM positions p
             JOIN assets a ON a.id = p.asset_id
             WHERE p.user_id = $1
             ORDER BY a.name;",
            user_id
        )
        .fetch_all(&self.db)
        .await
    }

    /// Registra uma compra. Se o ativo já estiver na carteira, soma a quantidade
    /// e recalcula o preço médio ponderado.
    pub async fn buy(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: f64,
        price: f64,
    ) -> sqlx::Result<Position> {
        sqlx::query_as!(
            Position,
            "INSERT INTO positions (user_id, asset_id, quantity, avg_price)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (user_id, asset_id) DO UPDATE
             SET avg_price = (positions.quantity * positions.avg_price
                              + EXCLUDED.quantity * EXCLUDED.avg_price)
                             / (positions.quantity + EXCLUDED.quantity),
                 quantity = positions.quantity + EXCLUDED.quantity
             RETURNING id, user_id, asset_id, quantity, avg_price;",
            user_id,
            asset_id,
            quantity,
            price
        )
        .fetch_one(&self.db)
        .await
    }

    /// Registra uma venda. O preço médio não muda; a posição é encerrada quando zera.
    pub async fn sell(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: f64,
    ) -> sqlx::Result<SellOutcome> {
        let mut tx = self.db.begin().await?;

        let current = sqlx::query_scalar!(
            "SELECT quantity
             FROM positions
             WHERE user_id = $1 AND asset_id = $2
             FOR UPDATE;",
            user_id,
            asset_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        let Some(current) = current else {
            return Ok(SellOutcome::NotFound);
        };

        if quantity > current + QUANTITY_EPSILON {
            return Ok(SellOutcome::InsufficientQuantity);
        }

        let remaining = current - quantity;
        let outcome = if remaining <= QUANTITY_EPSILON {
            sqlx::query!(
                "DELETE FROM positions WHERE user_id = $1 AND asset_id = $2;",
                user_id,
                asset_id
            )
            .execute(&mut *tx)
            .await?;
            SellOutcome::Closed
        } else {
            sqlx::query!(
                "UPDATE positions SET quantity = $3 WHERE user_id = $1 AND asset_id = $2;",
                user_id,
                asset_id,
                remaining
            )
            .execute(&mut *tx)
            .await?;
            SellOutcome::Reduced(remaining)
        };

        tx.commit().await?;
        Ok(outcome)
    }

    /// Remove uma posição, desde que ela pertença à pessoa usuária.
    pub async fn delete_position(&self, user_id: i64, position_id: i64) -> sqlx::Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM positions WHERE id = $1 AND user_id = $2;",
            position_id,
            user_id
        )
        .execute(&self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl FromRequestParts<AppState> for Repository {
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self {
            db: state.db.clone(),
        })
    }
}

#[cfg(test)]
impl From<PgPool> for Repository {
    fn from(db: PgPool) -> Self {
        Self { db }
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;

    const ALICE: i64 = 1;
    const BITCOIN: i64 = 1;

    #[sqlx::test(fixtures("bitcoin_asset", "user"))]
    async fn buying_twice_uses_weighted_average_price(db: PgPool) {
        let repository = Repository::from(db);

        repository.buy(ALICE, BITCOIN, 2.0, 100.0).await.unwrap();
        let position = repository.buy(ALICE, BITCOIN, 2.0, 200.0).await.unwrap();

        assert_eq!(position.quantity, 4.0);
        assert_eq!(position.avg_price, 150.0);
    }

    #[sqlx::test(fixtures("bitcoin_asset", "user"))]
    async fn selling_reduces_and_then_closes_the_position(db: PgPool) {
        let repository = Repository::from(db);
        repository.buy(ALICE, BITCOIN, 3.0, 100.0).await.unwrap();

        let outcome = repository.sell(ALICE, BITCOIN, 1.0).await.unwrap();
        assert_eq!(outcome, SellOutcome::Reduced(2.0));

        let rows = repository.list_positions(ALICE).await.unwrap();
        assert_eq!(rows[0].quantity, 2.0);
        assert_eq!(rows[0].avg_price, 100.0);

        let outcome = repository.sell(ALICE, BITCOIN, 2.0).await.unwrap();
        assert_eq!(outcome, SellOutcome::Closed);
        assert!(repository.list_positions(ALICE).await.unwrap().is_empty());
    }

    #[sqlx::test(fixtures("bitcoin_asset", "user"))]
    async fn cannot_sell_more_than_owned_or_unknown_positions(db: PgPool) {
        let repository = Repository::from(db);

        let outcome = repository.sell(ALICE, BITCOIN, 1.0).await.unwrap();
        assert_eq!(outcome, SellOutcome::NotFound);

        repository.buy(ALICE, BITCOIN, 1.0, 100.0).await.unwrap();
        let outcome = repository.sell(ALICE, BITCOIN, 1.5).await.unwrap();
        assert_eq!(outcome, SellOutcome::InsufficientQuantity);
    }

    #[sqlx::test(fixtures("bitcoin_asset", "user"))]
    async fn only_the_owner_can_delete_a_position(db: PgPool) {
        let repository = Repository::from(db);
        let position = repository.buy(ALICE, BITCOIN, 1.0, 100.0).await.unwrap();

        assert!(
            !repository
                .delete_position(ALICE + 1, position.id)
                .await
                .unwrap()
        );
        assert!(
            repository
                .delete_position(ALICE, position.id)
                .await
                .unwrap()
        );
        assert!(repository.list_positions(ALICE).await.unwrap().is_empty());
    }
}
