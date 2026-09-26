use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, SameSite},
};
use serde::Deserialize;

use crate::{
    app::AppState,
    auth::user::{SESSION_HOURS, TOKEN_COOKIE, UnauthenticatedUser, User},
    error::AppError,
    format,
    models::Asset,
    portfolio::{self, PortfolioSummary},
    repository::{Repository, SellOutcome},
    validation,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(dashboard))
        .route("/login", get(login_page).post(login))
        .route("/logout", post(logout))
        .route("/positions/buy", post(buy))
        .route("/positions/sell", post(sell))
        .route("/positions/{id}/delete", post(delete_position))
}

// ---------- Login ----------

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {
    error: Option<String>,
}

async fn login_page(maybe_user: Option<User>) -> Result<Response, AppError> {
    if maybe_user.is_some() {
        return Ok(Redirect::to("/").into_response());
    }
    let html = LoginPage { error: None }.render()?;
    Ok(Html(html).into_response())
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

fn login_error(message: &str) -> Result<Response, AppError> {
    let html = LoginPage {
        error: Some(message.to_string()),
    }
    .render()?;
    Ok((StatusCode::UNPROCESSABLE_ENTITY, Html(html)).into_response())
}

async fn login(
    State(state): State<AppState>,
    repository: Repository,
    jar: CookieJar,
    Form(request): Form<LoginForm>,
) -> Result<Response, AppError> {
    let username = request.username.trim().to_lowercase();
    if let Err(AppError::Validation(message)) =
        validation::validate_credentials(&username, &request.password)
    {
        return login_error(&message);
    }

    let unauth_user = UnauthenticatedUser::new(username, request.password);
    let user = match unauth_user.authenticate(&repository).await {
        Ok(user) => user,
        Err(AppError::UserDoesNotExist) => unauth_user.register(&repository).await?,
        Err(AppError::InvalidCredentials) => {
            return login_error("Usuário ou senha incorretos");
        }
        Err(other_err) => return Err(other_err),
    };

    let token = user.auth_token(&state.config.jwt_secret)?;
    let cookie = Cookie::build((TOKEN_COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::hours(SESSION_HOURS as i64));

    Ok((jar.add(cookie), Redirect::to("/")).into_response())
}

async fn logout(jar: CookieJar) -> impl IntoResponse {
    let jar = jar.remove(Cookie::build(TOKEN_COOKIE).path("/"));
    (jar, Redirect::to("/login"))
}

// ---------- Dashboard ----------

struct Flash {
    text: &'static str,
    success: bool,
}

/// Mensagens exibidas após um redirecionamento (`/?msg=codigo`).
fn flash_for(code: &str) -> Option<Flash> {
    let (text, success) = match code {
        "bought" => ("Compra registrada na sua carteira.", true),
        "sold" => ("Venda registrada.", true),
        "closed" => ("Posição encerrada: você vendeu tudo.", true),
        "removed" => ("Posição removida.", true),
        "invalid_trade" => (
            "Informe uma quantidade maior que zero e um preço válido.",
            false,
        ),
        "insufficient" => ("Você não tem essa quantidade para vender.", false),
        "not_found" => ("Ativo ou posição não encontrado.", false),
        _ => return None,
    };
    Some(Flash { text, success })
}

/// Tons do acento da marca para a alocação, do maior para o menor ativo.
/// Um acento só, em intensidades diferentes, funciona nos temas claro e escuro.
const PALETTE: [&str; 8] = [
    "var(--accent)",
    "color-mix(in srgb, var(--accent) 75%, var(--surface))",
    "color-mix(in srgb, var(--accent) 55%, var(--surface))",
    "color-mix(in srgb, var(--accent) 40%, var(--surface))",
    "color-mix(in srgb, var(--accent) 28%, var(--surface))",
    "color-mix(in srgb, var(--accent) 20%, var(--surface))",
    "color-mix(in srgb, var(--accent) 14%, var(--surface))",
    "color-mix(in srgb, var(--accent) 10%, var(--surface))",
];

struct HoldingView {
    position_id: i64,
    asset_id: i64,
    name: String,
    quantity: String,
    avg_price: String,
    unit_value: String,
    invested: String,
    market_value: String,
    pnl: String,
    pnl_pct: String,
    positive: bool,
    allocation: String,
    allocation_width: String,
    color: &'static str,
}

struct AssetOption {
    id: i64,
    name: String,
    price: String,
    raw_price: f64,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardPage {
    username: String,
    flash: Option<Flash>,
    market_value: String,
    total_invested: String,
    pnl: String,
    pnl_pct: String,
    positive: bool,
    holdings: Vec<HoldingView>,
    assets: Vec<AssetOption>,
}

impl DashboardPage {
    fn new(
        username: String,
        summary: PortfolioSummary,
        assets: Vec<Asset>,
        flash: Option<Flash>,
    ) -> Self {
        let holdings = summary
            .holdings
            .into_iter()
            .enumerate()
            .map(|(index, holding)| HoldingView {
                position_id: holding.position_id,
                asset_id: holding.asset_id,
                name: holding.asset_name,
                quantity: format::quantity(holding.quantity),
                avg_price: format::brl(holding.avg_price),
                unit_value: format::brl(holding.unit_value),
                invested: format::brl(holding.invested),
                market_value: format::brl(holding.market_value),
                pnl: format::brl(holding.pnl),
                pnl_pct: format::signed_percent(holding.pnl_pct),
                positive: holding.pnl >= 0.0,
                allocation: format!("{:.1}%", holding.allocation_pct).replace('.', ","),
                allocation_width: format!("{:.2}", holding.allocation_pct),
                color: PALETTE[index % PALETTE.len()],
            })
            .collect();

        let assets = assets
            .into_iter()
            .map(|asset| AssetOption {
                id: asset.id,
                price: format::brl(asset.unit_value),
                raw_price: asset.unit_value,
                name: asset.name,
            })
            .collect();

        Self {
            username,
            flash,
            market_value: format::brl(summary.market_value),
            total_invested: format::brl(summary.total_invested),
            pnl: format::brl(summary.pnl),
            pnl_pct: format::signed_percent(summary.pnl_pct),
            positive: summary.pnl >= 0.0,
            holdings,
            assets,
        }
    }
}

#[derive(Deserialize)]
struct DashboardQuery {
    msg: Option<String>,
}

async fn dashboard(
    maybe_user: Option<User>,
    repository: Repository,
    Query(query): Query<DashboardQuery>,
) -> Result<Response, AppError> {
    let Some(user) = maybe_user else {
        return Ok(Redirect::to("/login").into_response());
    };

    let summary = portfolio::summarize(repository.list_positions(user.id()).await?);
    let assets = repository.list_assets().await?;
    let flash = query.msg.as_deref().and_then(flash_for);

    let page = DashboardPage::new(user.username().clone(), summary, assets, flash);
    Ok(Html(page.render()?).into_response())
}

// ---------- Operações ----------

fn back_with(code: &str) -> Response {
    Redirect::to(&format!("/?msg={code}")).into_response()
}

#[derive(Deserialize)]
struct BuyForm {
    asset_id: i64,
    quantity: f64,
    price: f64,
}

async fn buy(
    user: Option<User>,
    repository: Repository,
    Form(form): Form<BuyForm>,
) -> Result<Response, AppError> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    if validation::validate_quantity(form.quantity).is_err()
        || validation::validate_price(form.price).is_err()
    {
        return Ok(back_with("invalid_trade"));
    }

    match repository
        .buy(user.id(), form.asset_id, form.quantity, form.price)
        .await
    {
        Ok(_) => Ok(back_with("bought")),
        Err(sqlx::Error::Database(db_err)) if db_err.is_foreign_key_violation() => {
            Ok(back_with("not_found"))
        }
        Err(err) => Err(err.into()),
    }
}

#[derive(Deserialize)]
struct SellForm {
    asset_id: i64,
    quantity: f64,
}

async fn sell(
    user: Option<User>,
    repository: Repository,
    Form(form): Form<SellForm>,
) -> Result<Response, AppError> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    if validation::validate_quantity(form.quantity).is_err() {
        return Ok(back_with("invalid_trade"));
    }

    let code = match repository
        .sell(user.id(), form.asset_id, form.quantity)
        .await?
    {
        SellOutcome::Reduced(_) => "sold",
        SellOutcome::Closed => "closed",
        SellOutcome::NotFound => "not_found",
        SellOutcome::InsufficientQuantity => "insufficient",
    };
    Ok(back_with(code))
}

async fn delete_position(
    user: Option<User>,
    repository: Repository,
    Path(position_id): Path<i64>,
) -> Result<Response, AppError> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };

    if repository.delete_position(user.id(), position_id).await? {
        Ok(back_with("removed"))
    } else {
        Ok(back_with("not_found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PositionRow;

    #[test]
    fn unknown_flash_codes_are_ignored() {
        assert!(flash_for("<script>").is_none());
        assert!(flash_for("bought").unwrap().success);
        assert!(!flash_for("insufficient").unwrap().success);
    }

    #[test]
    fn dashboard_renders_holdings_and_totals() {
        let summary = portfolio::summarize(vec![PositionRow {
            position_id: 1,
            asset_id: 1,
            asset_name: "PETR4".to_string(),
            quantity: 100.0,
            avg_price: 30.0,
            unit_value: 40.0,
        }]);
        let assets = vec![Asset {
            id: 1,
            name: "PETR4".to_string(),
            unit_value: 40.0,
        }];

        let html = DashboardPage::new("ana".to_string(), summary, assets, flash_for("bought"))
            .render()
            .unwrap();

        assert!(html.contains("ana"));
        assert!(html.contains("PETR4"));
        assert!(html.contains("R$ 4.000,00"));
        assert!(html.contains("+33,33%"));
        assert!(html.contains("Compra registrada"));
    }

    #[test]
    fn empty_dashboard_shows_onboarding() {
        let html = DashboardPage::new(
            "ana".to_string(),
            portfolio::summarize(vec![]),
            vec![],
            None,
        )
        .render()
        .unwrap();

        assert!(html.contains("Sua carteira está vazia"));
    }

    #[test]
    fn login_page_shows_errors() {
        let html = LoginPage {
            error: Some("Usuário ou senha incorretos".to_string()),
        }
        .render()
        .unwrap();

        assert!(html.contains("Usuário ou senha incorretos"));
    }
}
