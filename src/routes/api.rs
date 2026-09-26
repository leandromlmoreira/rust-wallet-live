use axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};

use crate::{
    app::AppState,
    auth::{admin::Admin, user::User},
    error::AppError,
    models::{Asset, TransactionRow},
    portfolio::{self, PortfolioSummary, TimelinePoint},
    repository::Repository,
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/assets",
            get(list_assets).post(create_asset).patch(update_asset),
        )
        .route("/portfolio", get(portfolio_summary))
        .route("/transactions", get(transactions))
}

#[tracing::instrument(skip_all)]
async fn list_assets(repostiory: Repository) -> Result<Json<Vec<Asset>>, AppError> {
    let assets = repostiory.list_assets().await?;
    Ok(Json(assets))
}

#[derive(Deserialize)]
struct CreateAssetRequest {
    name: String,
    unit_value: f64,
}

#[tracing::instrument(skip_all)]
async fn create_asset(
    _: Admin,
    repostiory: Repository,
    Json(request): Json<CreateAssetRequest>,
) -> Result<Json<Asset>, AppError> {
    validation::validate_asset_name(&request.name)?;
    validation::validate_unit_value(request.unit_value)?;

    let new_asset = repostiory
        .create_asset(request.name.trim().to_string(), request.unit_value)
        .await
        .map_err(|err| match err {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                AppError::Validation("Já existe um ativo com esse nome".to_string())
            }
            other => AppError::Database(other),
        })?;

    Ok(Json(new_asset))
}

#[derive(Deserialize)]
struct UpdateAssetRequest {
    id: i64,
    name: Option<String>,
    unit_value: Option<f64>,
}

#[tracing::instrument(skip_all)]
async fn update_asset(
    _: Admin,
    repostiory: Repository,
    Json(request): Json<UpdateAssetRequest>,
) -> Result<Json<Asset>, AppError> {
    if let Some(name) = &request.name {
        validation::validate_asset_name(name)?;
    }
    if let Some(unit_value) = request.unit_value {
        validation::validate_unit_value(unit_value)?;
    }

    let name = request.name.map(|name| name.trim().to_string());
    match repostiory
        .update_asset(request.id, name, request.unit_value)
        .await?
    {
        Some(updated_asset) => Ok(Json(updated_asset)),
        None => Err(AppError::AssetDoesNotExist),
    }
}

/// Resumo da carteira da pessoa autenticada (cookie de sessão).
#[tracing::instrument(skip_all)]
async fn portfolio_summary(
    user: User,
    repository: Repository,
) -> Result<Json<PortfolioSummary>, AppError> {
    let rows = repository.list_positions(user.id()).await?;
    Ok(Json(portfolio::summarize(rows)))
}

#[derive(Serialize)]
struct TransactionsResponse {
    transactions: Vec<TransactionRow>,
    realized_pnl: f64,
    invested_timeline: Vec<TimelinePoint>,
}

/// Histórico de operações, lucro realizado e evolução do capital investido.
#[tracing::instrument(skip_all)]
async fn transactions(
    user: User,
    repository: Repository,
) -> Result<Json<TransactionsResponse>, AppError> {
    let transactions = repository.list_transactions(user.id()).await?;
    Ok(Json(TransactionsResponse {
        realized_pnl: portfolio::realized_pnl(&transactions),
        invested_timeline: portfolio::invested_timeline(&transactions),
        transactions,
    }))
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;

    #[sqlx::test]
    async fn test_create_asset(db: PgPool) {
        let request = CreateAssetRequest {
            name: "Bitcoin".to_string(),
            unit_value: 10.0,
        };
        let Json(new_asset) = create_asset(Admin, db.into(), Json(request))
            .await
            .expect("success");

        assert_eq!(new_asset.id, 1);
        assert_eq!(new_asset.name, "Bitcoin");
        assert_eq!(new_asset.unit_value, 10.0);

        insta::assert_json_snapshot!(new_asset);
    }

    #[sqlx::test]
    async fn test_create_asset_rejects_invalid_values(db: PgPool) {
        let request = CreateAssetRequest {
            name: "  ".to_string(),
            unit_value: -5.0,
        };
        let result = create_asset(Admin, db.into(), Json(request)).await;

        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[sqlx::test(fixtures("bitcoin_asset"))]
    async fn test_create_asset_rejects_duplicated_name(db: PgPool) {
        let request = CreateAssetRequest {
            name: "Bitcoin".to_string(),
            unit_value: 1.0,
        };
        let result = create_asset(Admin, db.into(), Json(request)).await;

        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[sqlx::test(fixtures("bitcoin_asset"))]
    async fn test_list_assets(db: PgPool) {
        let Json(assets) = list_assets(db.into()).await.expect("success");

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].name, "Bitcoin");

        insta::assert_json_snapshot!(assets);
    }

    #[sqlx::test(fixtures("bitcoin_asset"))]
    async fn test_update_asset(db: PgPool) {
        let request = UpdateAssetRequest {
            id: 1,
            name: Some("Ethereum".to_string()),
            unit_value: Some(20.0),
        };

        let Json(updated_asset) = update_asset(Admin, db.into(), Json(request))
            .await
            .expect("success");

        assert_eq!(updated_asset.id, 1);
        assert_eq!(updated_asset.name, "Ethereum");
        assert_eq!(updated_asset.unit_value, 20.0);

        insta::assert_json_snapshot!(updated_asset);
    }

    #[sqlx::test]
    async fn test_update_missing_asset(db: PgPool) {
        let request = UpdateAssetRequest {
            id: 42,
            name: None,
            unit_value: Some(1.0),
        };
        let result = update_asset(Admin, db.into(), Json(request)).await;

        assert!(matches!(result, Err(AppError::AssetDoesNotExist)));
    }
}
