# 🦀 Wallet Live - Carteira de Investimentos Inteligente em Rust

![CI](../../actions/workflows/ci.yml/badge.svg)

Aplicação **fullstack em Rust** para acompanhar uma carteira de investimentos: API REST, banco PostgreSQL, autenticação com JWT em cookie e dashboard web renderizado no servidor.

Projeto final do **Bootcamp Santander 2026 - Rust AI Developer (DIO)**, construído a partir do [repositório base](https://github.com/digitalinnovationone/rust-fullstack-carteira-investimentos) e evoluído com uma carteira completa por pessoa usuária.

---

## ✨ O que o projeto faz

| Recurso | Descrição |
|---|---|
| **Login / cadastro** | A conta é criada no primeiro acesso; senhas com hash (`password-auth`) e sessão por JWT em cookie `HttpOnly` + `SameSite=Lax` (8h). |
| **Catálogo de ativos** | Admin cadastra e atualiza ativos e cotações pela API (`/api/assets`). |
| **Carteira por usuário** ⭐ | Cada pessoa registra **compras** e **vendas** dos seus ativos. |
| **Preço médio ponderado** ⭐ | Comprar um ativo que já está na carteira recalcula o preço médio automaticamente (no próprio `UPSERT` do Postgres). |
| **Vendas seguras** ⭐ | Venda em transação com `SELECT … FOR UPDATE`; não permite vender mais do que se tem e encerra a posição ao zerar. |
| **Dashboard** ⭐ | Valor de mercado, total investido, lucro/prejuízo (R$ e %), barra de alocação colorida e tabela por ativo - tudo formatado em padrão brasileiro. |
| **API de resumo** ⭐ | `GET /api/portfolio` devolve o resumo da carteira da pessoa logada em JSON. |

⭐ = melhorias implementadas nesta entrega.

## 🧱 Tecnologias

- **Rust 2024** · **Axum 0.8** (rotas e extractors)
- **SQLx 0.8 + PostgreSQL** (queries verificadas em tempo de compilação, migrations, `#[sqlx::test]`)
- **Askama** (templates HTML compilados) + Tailwind via CDN
- **jwt-simple** (HS256, implementação pure-Rust) · **password-auth** (hash de senha)
- **insta** (snapshot tests) · **GitHub Actions** (fmt + clippy + testes com Postgres)

## 🗂️ Estrutura

```
src/
├── app.rs            # AppState, Config (segredos via ambiente), migrations e servidor
├── auth/             # Admin (header Authorization) e User (JWT em cookie)
├── routes/
│   ├── api.rs        # /api/assets e /api/portfolio
│   └── frontend.rs   # login, logout, dashboard, compra/venda/remoção
├── repository.rs     # acesso ao banco (assets, users, positions)
├── portfolio.rs      # cálculos puros: investido, valor, P&L, alocação
├── validation.rs     # regras de entrada compartilhadas
├── format.rs         # R$ 1.234,56 · +12,34% · quantidades
└── error.rs          # AppError → respostas HTTP
templates/            # login.html e dashboard.html (Askama)
migrations/           # assets, users e positions
seeds/assets.sql      # ativos de exemplo
```

## ▶️ Como executar

Pré-requisitos: **Rust** (stable) e **PostgreSQL** (local ou via Docker).

```bash
# 1. Banco de dados (opção Docker)
docker compose up -d

# 2. Variáveis de ambiente (já existe um .env de desenvolvimento)
cp .env.example .env   # ajuste JWT_SECRET e ADMIN_KEY em produção

# 3. Rodar (as migrations são aplicadas automaticamente na inicialização)
cargo run

# 4. (opcional) ativos de exemplo
psql postgres://postgres:postgres@localhost:5432/postgres -f seeds/assets.sql
```

Acesse **http://127.0.0.1:3000**, escolha um usuário e uma senha (a conta é criada no primeiro acesso) e registre suas compras.

| Variável | Uso |
|---|---|
| `DATABASE_URL` | conexão com o PostgreSQL |
| `JWT_SECRET` | chave de assinatura dos tokens (mín. 16 caracteres) |
| `ADMIN_KEY` | valor esperado no header `Authorization` das rotas de admin |
| `PORT` | porta HTTP (padrão `3000`) |

### Exemplos de API

```bash
# Cadastrar um ativo (admin)
curl -X POST http://127.0.0.1:3000/api/assets \
  -H "Authorization: im-the-admin" -H "Content-Type: application/json" \
  -d '{"name": "PETR4", "unit_value": 38.40}'

# Atualizar a cotação (admin)
curl -X PATCH http://127.0.0.1:3000/api/assets \
  -H "Authorization: im-the-admin" -H "Content-Type: application/json" \
  -d '{"id": 1, "unit_value": 40.10}'

# Listar ativos
curl http://127.0.0.1:3000/api/assets

# Resumo da minha carteira (usa o cookie de sessão do navegador)
curl http://127.0.0.1:3000/api/portfolio -H "Cookie: token=<seu-token>"
```

Entradas inválidas (valor ≤ 0, nome vazio ou repetido, quantidade negativa) retornam **422** com uma mensagem clara.

## 🧪 Como testar

```bash
cargo test                                   # 26 testes (unitários + banco)
cargo clippy --all-targets -- -D warnings    # lint sem avisos
cargo fmt --check
```

Os testes com `#[sqlx::test]` criam um banco temporário por teste, aplicam as migrations e carregam fixtures - por isso precisam de um PostgreSQL acessível em `DATABASE_URL`.

O que é coberto:

- **Cálculos da carteira** - totais, P&L, alocação somando 100%, posições com custo zero
- **Repositório** - preço médio ponderado, venda parcial/total, venda acima do saldo, posição de outra pessoa
- **API** - criação, listagem, atualização, validações e nome duplicado (com snapshots `insta`)
- **Autenticação** - ida e volta do JWT e rejeição de token assinado com outra chave
- **Páginas** - dashboard com dados, carteira vazia, mensagens de erro no login
- **Formatação e validação** - moeda, percentual, quantidades, credenciais

O **GitHub Actions** roda tudo isso a cada push, com um serviço PostgreSQL.

## 🚀 Melhorias implementadas (em relação ao projeto base)

1. **Carteira por usuário** - nova tabela `positions` (FK para `users` e `assets`, `UNIQUE(user_id, asset_id)` e `CHECK`s de quantidade/preço).
2. **Compra com preço médio ponderado** calculado atomicamente no `INSERT … ON CONFLICT DO UPDATE`.
3. **Venda transacional** com bloqueio de linha, tolerância para frações e encerramento automático da posição.
4. **Dashboard completo** substituindo o antigo `Hello, <usuário>`: cards de resumo, barra de alocação, tabela de posições, formulários de compra/venda e logout.
5. **Segurança**: segredos saíram do código para variáveis de ambiente; cookie com `Path`, `SameSite` e expiração; hash corrompido não derruba mais o servidor (antes havia um `panic!`); uma pessoa não consegue apagar a posição de outra.
6. **Validações e erros amigáveis** na API (422) e nas páginas (mensagens em português).
7. **Migrations automáticas** na inicialização e `PORT` configurável.
8. **Testes**: de 3 para 26, e **CI** no GitHub Actions (fmt, clippy e testes).

## 📚 O que aprendi

- Como o **sistema de tipos do Rust** ajuda numa API web: extractors do Axum (`User`, `Admin`, `Repository`) transformam autenticação e acesso a dados em parâmetros de função, e o compilador impede rotas sem as dependências certas.
- O valor do **SQLx com queries checadas em compilação**: um erro de coluna aparece no `cargo build`, não em produção.
- Deixar a **regra de negócio no lugar certo**: o preço médio no banco (atômico) e os cálculos de exibição em funções puras (`portfolio.rs`), fáceis de testar sem banco.
- **Transações e concorrência** (`FOR UPDATE`) para evitar vender a mesma posição duas vezes.
- **Boas práticas de sessão**: JWT em cookie `HttpOnly`/`SameSite`, segredos fora do código e mensagens de erro que não vazam detalhes.
- Montar uma **esteira de qualidade** (fmt, clippy, testes com banco real no CI).

---

Feito com 🦀 durante o Bootcamp Santander 2026 - Rust AI Developer, na [DIO](https://www.dio.me).
