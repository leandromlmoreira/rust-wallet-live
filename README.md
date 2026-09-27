# Wallet Live

**Carteira de investimentos fullstack em Rust: registre compras e vendas, acompanhe preço médio, lucro realizado e a evolução do patrimônio num painel renderizado no servidor.**

[![CI](https://github.com/leandromlmoreira/rust-wallet-live/actions/workflows/ci.yml/badge.svg)](https://github.com/leandromlmoreira/rust-wallet-live/actions/workflows/ci.yml)
![Rust 2024](https://img.shields.io/badge/Rust-2024-b7410e)
![Axum 0.8](https://img.shields.io/badge/Axum-0.8-1f2937)
![PostgreSQL](https://img.shields.io/badge/PostgreSQL-SQLx-336791)

![Painel do Wallet Live com resumo da carteira, capital investido, alocação e resultado por ativo](docs/preview.png)

<table>
  <tr>
    <td width="68%"><img src="docs/preview-dark.png" alt="Painel no tema escuro"></td>
    <td width="32%"><img src="docs/preview-mobile.png" alt="Painel em tela de celular"></td>
  </tr>
  <tr>
    <td align="center"><sub>Tema escuro automático</sub></td>
    <td align="center"><sub>Layout em 375 px</sub></td>
  </tr>
</table>

<sub>Capturas do app rodando localmente (`cargo run` + PostgreSQL) com uma carteira de demonstração criada pelo próprio formulário de operações.</sub>

## Funcionalidades

- **Conta própria em um passo**: a conta é criada no primeiro login; senha com hash (`password-auth`) e sessão por JWT em cookie `HttpOnly`, `SameSite=Lax`, válido por 8 horas.
- **Compras com preço médio ponderado**: calculado de forma atômica no `INSERT … ON CONFLICT DO UPDATE` do PostgreSQL.
- **Vendas seguras**: transação com `SELECT … FOR UPDATE`, bloqueio de venda acima do saldo e encerramento automático da posição zerada.
- **Histórico e lucro realizado**: cada operação grava data, preço e preço médio do momento; o lucro de cada venda é `quantidade × (preço de venda − preço médio)`.
- **Painel analítico**: valor de mercado, total investido, resultado em aberto e realizado, e três gráficos SVG gerados em Rust (capital ao longo do tempo, alocação em rosca e resultado por ativo), com tooltips por mouse e teclado.
- **Tema claro e escuro** pelo sistema, com paleta categórica validada para daltonismo e cor fixa por ativo.
- **Exportação CSV** no padrão do Excel em português (`;` e vírgula decimal), protegida contra injeção de fórmulas.
- **API REST** para ativos, resumo da carteira e histórico, com erros `422` claros em entradas inválidas.

## Arquitetura

```mermaid
flowchart LR
    B[Navegador] -- "HTML (Askama) + SVG" --> F[routes/frontend.rs]
    C[Cliente HTTP] -- JSON --> A[routes/api.rs]
    F --> X[auth: User via cookie JWT / Admin via header]
    A --> X
    F --> P[portfolio.rs<br/>cálculos puros]
    F --> G[charts.rs<br/>geometria dos gráficos]
    A --> P
    F --> R[repository.rs]
    A --> R
    R -- "SQLx (queries checadas em compilação)" --> D[(PostgreSQL)]
```

| Camada | Responsabilidade |
|---|---|
| `app.rs` | estado da aplicação, configuração por ambiente, migrations automáticas e servidor |
| `auth/` | extractors `User` (JWT em cookie) e `Admin` (header `Authorization`) |
| `routes/frontend.rs` | login, painel, compra, venda, remoção e exportação CSV |
| `routes/api.rs` | `/api/assets`, `/api/portfolio` e `/api/transactions` |
| `repository.rs` | acesso ao banco: ativos, usuários, posições e transações |
| `portfolio.rs` | P&L, alocação, lucro realizado e linha do tempo, sem I/O |
| `charts.rs` | escalas, linha, rosca e barras divergentes |
| `format.rs` / `validation.rs` / `dates.rs` | moeda brasileira, regras de entrada e datas em horário de Brasília |
| `templates/` | `base.html` (tokens de tema), `login.html` e `dashboard.html` |

A regra de negócio que precisa de atomicidade (preço médio, venda) fica no banco; tudo o que é exibição fica em funções puras, testáveis sem PostgreSQL.

### Rotas

| Método | Rota | Acesso |
|---|---|---|
| `GET` | `/` | painel da pessoa logada |
| `GET` `POST` | `/login` | página e envio do login |
| `POST` | `/logout` | encerra a sessão |
| `POST` | `/positions/buy` · `/positions/sell` | registra compra ou venda |
| `POST` | `/positions/{id}/delete` | remove uma posição própria |
| `GET` | `/export/operacoes.csv` | exporta o histórico |
| `GET` | `/api/assets` | lista ativos |
| `POST` `PATCH` | `/api/assets` | cadastra ou atualiza cotação (admin) |
| `GET` | `/api/portfolio` · `/api/transactions` | resumo e histórico (sessão) |

## Stack

Rust 2024 · Axum 0.8 · SQLx 0.8 + PostgreSQL · Askama · jwt-simple · password-auth · time · insta · GitHub Actions

## Como rodar

Pré-requisitos: Rust stable e PostgreSQL (local ou Docker).

```bash
docker compose up -d
cp .env.example .env
cargo run
psql postgres://postgres:postgres@localhost:5432/postgres -f seeds/assets.sql
```

As migrations rodam na inicialização. Acesse http://127.0.0.1:3000, escolha usuário e senha (a conta nasce no primeiro acesso) e registre as operações.

| Variável | Uso |
|---|---|
| `DATABASE_URL` | conexão com o PostgreSQL |
| `JWT_SECRET` | chave de assinatura dos tokens (mínimo 16 caracteres) |
| `ADMIN_KEY` | valor esperado no header `Authorization` das rotas de admin |
| `PORT` | porta HTTP (padrão `3000`) |

Os valores do `.env` versionado servem só para desenvolvimento; em produção defina segredos próprios.

```bash
curl -X POST http://127.0.0.1:3000/api/assets \
  -H "Authorization: im-the-admin" -H "Content-Type: application/json" \
  -d '{"name": "PETR4", "unit_value": 38.40}'

curl http://127.0.0.1:3000/api/transactions -H "Cookie: token=<seu-token>"
```

## Qualidade

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

São 38 testes, entre unitários e de banco (`#[sqlx::test]` cria um banco temporário por teste, aplica as migrations e carrega fixtures). Cobrem cálculos da carteira, geometria dos gráficos, preço médio e vendas parciais, bloqueio de posição alheia, CSV e injeção de fórmulas, validações da API com snapshots `insta`, JWT e renderização das páginas.

O workflow [`ci.yml`](.github/workflows/ci.yml) roda formatação, clippy e testes contra um serviço PostgreSQL a cada push e pull request.

---

<sub>Evolução do projeto final do Bootcamp Santander 2026 · Rust AI Developer (DIO), a partir do [repositório base](https://github.com/digitalinnovationone/rust-fullstack-carteira-investimentos).</sub>
