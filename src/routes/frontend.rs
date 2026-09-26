use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
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
    charts::{self, DonutSlice, LineChart, ResultBar},
    dates,
    error::AppError,
    format,
    models::{Asset, TransactionRow},
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
        .route("/export/operacoes.csv", get(export_csv))
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
        "removed" => ("Posição e histórico do ativo removidos.", true),
        "invalid_trade" => (
            "Informe uma quantidade maior que zero e um preço válido.",
            false,
        ),
        "invalid_date" => ("Informe uma data válida, que não esteja no futuro.", false),
        "insufficient" => ("Você não tem essa quantidade para vender.", false),
        "not_found" => ("Ativo ou posição não encontrado.", false),
        _ => return None,
    };
    Some(Flash { text, success })
}

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
    color: &'static str,
    raw_unit_value: f64,
}

struct AssetOption {
    id: i64,
    name: String,
    price: String,
    raw_price: f64,
}

struct TransactionView {
    date: String,
    is_buy: bool,
    asset: String,
    quantity: String,
    price: String,
    total: String,
    /// Resultado realizado (só para vendas).
    realized: Option<(String, bool)>,
}

impl TransactionView {
    fn new(transaction: &TransactionRow) -> Self {
        let is_buy = transaction.kind == "buy";
        let realized = (!is_buy).then(|| {
            let value = transaction.quantity * (transaction.price - transaction.cost_basis);
            (format::brl(value), value >= 0.0)
        });
        Self {
            date: dates::parse_iso(&transaction.executed_on)
                .map(dates::br)
                .unwrap_or_else(|| transaction.executed_on.clone()),
            is_buy,
            asset: transaction.asset_name.clone(),
            quantity: format::quantity(transaction.quantity),
            price: format::brl(transaction.price),
            total: format::brl(transaction.quantity * transaction.price),
            realized,
        }
    }
}

/// Quantas operações recentes aparecem na página (o CSV traz todas).
const RECENT_OPERATIONS: usize = 8;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardPage {
    username: String,
    flash: Option<Flash>,
    today: String,
    market_value: String,
    total_invested: String,
    pnl: String,
    pnl_pct: String,
    positive: bool,
    realized: String,
    realized_positive: bool,
    operations_count: usize,
    holdings: Vec<HoldingView>,
    assets: Vec<AssetOption>,
    timeline: Option<LineChart>,
    donut: Vec<DonutSlice>,
    bars: Vec<ResultBar>,
    recent: Vec<TransactionView>,
}

impl DashboardPage {
    fn new(
        username: String,
        summary: PortfolioSummary,
        assets: Vec<Asset>,
        transactions: Vec<TransactionRow>,
        flash: Option<Flash>,
    ) -> Self {
        let today = dates::today();
        let timeline = charts::invested_line(&portfolio::invested_timeline(&transactions), today);
        let donut = charts::allocation_donut(&summary.holdings);
        let bars = charts::result_bars(&summary.holdings);
        let realized = portfolio::realized_pnl(&transactions);
        let recent = transactions
            .iter()
            .rev()
            .take(RECENT_OPERATIONS)
            .map(TransactionView::new)
            .collect();

        let holdings = summary
            .holdings
            .into_iter()
            .map(|holding| HoldingView {
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
                allocation: format::percent(holding.allocation_pct),
                color: charts::series_color(holding.color_slot),
                raw_unit_value: holding.unit_value,
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
            today: dates::iso(today),
            market_value: format::brl(summary.market_value),
            total_invested: format::brl(summary.total_invested),
            pnl: format::brl(summary.pnl),
            pnl_pct: format::signed_percent(summary.pnl_pct),
            positive: summary.pnl >= 0.0,
            realized: format::brl(realized),
            realized_positive: realized >= 0.0,
            operations_count: transactions.len(),
            holdings,
            assets,
            timeline,
            donut,
            bars,
            recent,
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
    let transactions = repository.list_transactions(user.id()).await?;
    let flash = query.msg.as_deref().and_then(flash_for);

    let page = DashboardPage::new(
        user.username().clone(),
        summary,
        assets,
        transactions,
        flash,
    );
    Ok(Html(page.render()?).into_response())
}

// ---------- Operações ----------

fn back_with(code: &str) -> Response {
    Redirect::to(&format!("/?msg={code}")).into_response()
}

#[derive(Deserialize)]
struct TradeForm {
    asset_id: i64,
    quantity: f64,
    price: f64,
    executed_on: String,
}

/// Valida um formulário de operação. Devolve a data normalizada ou o código de erro.
fn check_trade(form: &TradeForm) -> Result<String, &'static str> {
    if validation::validate_quantity(form.quantity).is_err()
        || validation::validate_price(form.price).is_err()
    {
        return Err("invalid_trade");
    }
    validation::validate_trade_date(&form.executed_on).map_err(|_| "invalid_date")
}

async fn buy(
    user: Option<User>,
    repository: Repository,
    Form(form): Form<TradeForm>,
) -> Result<Response, AppError> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let executed_on = match check_trade(&form) {
        Ok(date) => date,
        Err(code) => return Ok(back_with(code)),
    };

    match repository
        .buy(
            user.id(),
            form.asset_id,
            form.quantity,
            form.price,
            &executed_on,
        )
        .await
    {
        Ok(_) => Ok(back_with("bought")),
        Err(sqlx::Error::Database(db_err)) if db_err.is_foreign_key_violation() => {
            Ok(back_with("not_found"))
        }
        Err(err) => Err(err.into()),
    }
}

async fn sell(
    user: Option<User>,
    repository: Repository,
    Form(form): Form<TradeForm>,
) -> Result<Response, AppError> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let executed_on = match check_trade(&form) {
        Ok(date) => date,
        Err(code) => return Ok(back_with(code)),
    };

    let code = match repository
        .sell(
            user.id(),
            form.asset_id,
            form.quantity,
            form.price,
            &executed_on,
        )
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

// ---------- Exportação ----------

/// Campo de CSV seguro: aspas quando preciso e proteção contra fórmulas no Excel.
fn csv_field(text: &str) -> String {
    let text = if text.starts_with(['=', '+', '-', '@']) {
        format!("'{text}")
    } else {
        text.to_string()
    };
    if text.contains([';', '"', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

fn decimal(value: f64) -> String {
    let text = format!("{value:.8}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.replace('.', ",")
}

/// CSV no padrão do Excel em português: `;` como separador e vírgula decimal.
fn transactions_csv(transactions: &[TransactionRow]) -> String {
    let mut csv = String::from(
        "data;operacao;ativo;quantidade;preco;total;preco_medio;resultado_realizado\n",
    );
    for t in transactions {
        let is_sell = t.kind == "sell";
        let realized = if is_sell {
            decimal(t.quantity * (t.price - t.cost_basis))
        } else {
            String::new()
        };
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{}\n",
            t.executed_on,
            if is_sell { "venda" } else { "compra" },
            csv_field(&t.asset_name),
            decimal(t.quantity),
            decimal(t.price),
            decimal(t.quantity * t.price),
            decimal(t.cost_basis),
            realized
        ));
    }
    csv
}

async fn export_csv(user: Option<User>, repository: Repository) -> Result<Response, AppError> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let transactions = repository.list_transactions(user.id()).await?;
    // BOM para o Excel reconhecer UTF-8 (acentos nos nomes dos ativos).
    let body = format!("\u{feff}{}", transactions_csv(&transactions));

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"operacoes-wallet-live.csv\"",
            ),
        ],
        body,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PositionRow;

    fn transaction(id: i64, kind: &str, day: &str, price: f64) -> TransactionRow {
        TransactionRow {
            id,
            asset_id: 1,
            asset_name: "PETR4".to_string(),
            kind: kind.to_string(),
            quantity: 100.0,
            price,
            cost_basis: 30.0,
            executed_on: day.to_string(),
        }
    }

    fn sample_page(flash: Option<Flash>) -> DashboardPage {
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
        let history = vec![
            transaction(1, "buy", "2026-01-05", 30.0),
            transaction(2, "buy", "2026-03-10", 30.0),
            transaction(3, "sell", "2026-06-01", 35.0),
        ];
        DashboardPage::new("ana".to_string(), summary, assets, history, flash)
    }

    #[test]
    fn unknown_flash_codes_are_ignored() {
        assert!(flash_for("<script>").is_none());
        assert!(flash_for("bought").unwrap().success);
        assert!(!flash_for("insufficient").unwrap().success);
    }

    #[test]
    fn dashboard_renders_totals_charts_and_history() {
        let html = sample_page(flash_for("bought")).render().unwrap();

        assert!(html.contains("ana"));
        assert!(html.contains("R$ 4.000,00"));
        assert!(html.contains("+33,33%"));
        assert!(html.contains("Compra registrada"));
        // Lucro realizado: 100 x (35 - 30)
        assert!(html.contains("R$ 500,00"));
        // Gráficos e tabela de apoio
        assert!(html.contains("id=\"chart-invested\""));
        assert!(html.contains("id=\"chart-allocation\""));
        assert!(html.contains("id=\"chart-results\""));
        assert!(html.contains("01/06/2026"));
    }

    #[test]
    fn empty_dashboard_shows_onboarding() {
        let html = DashboardPage::new(
            "ana".to_string(),
            portfolio::summarize(vec![]),
            vec![],
            vec![],
            None,
        )
        .render()
        .unwrap();

        assert!(html.contains("Sua carteira está vazia"));
        assert!(!html.contains("id=\"chart-allocation\""));
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

    #[test]
    fn csv_uses_brazilian_excel_format_and_blocks_formulas() {
        let mut history = vec![
            transaction(1, "buy", "2026-01-05", 30.5),
            transaction(2, "sell", "2026-06-01", 35.0),
        ];
        history[1].asset_name = "=HYPERLINK(\"x\")".to_string();

        let csv = transactions_csv(&history);
        let lines: Vec<&str> = csv.lines().collect();

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], "2026-01-05;compra;PETR4;100;30,5;3050;30;");
        assert!(lines[2].starts_with("2026-06-01;venda;\"'=HYPERLINK(\"\"x\"\")\";"));
        assert!(lines[2].ends_with(";500"));
    }

    #[test]
    fn trade_form_rejects_future_dates() {
        let form = TradeForm {
            asset_id: 1,
            quantity: 1.0,
            price: 1.0,
            executed_on: "2999-01-01".to_string(),
        };
        assert_eq!(check_trade(&form), Err("invalid_date"));
    }
}
