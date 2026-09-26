CREATE TABLE IF NOT EXISTS transactions (
 id BIGSERIAL PRIMARY KEY NOT NULL,
 user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
 asset_id BIGINT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
 kind TEXT NOT NULL CHECK (kind IN ('buy', 'sell')),
 quantity DOUBLE PRECISION NOT NULL CHECK (quantity > 0),
 price DOUBLE PRECISION NOT NULL CHECK (price >= 0),
 -- Preço médio da posição no momento da operação (base para o lucro realizado).
 cost_basis DOUBLE PRECISION NOT NULL CHECK (cost_basis >= 0),
 executed_on DATE NOT NULL DEFAULT CURRENT_DATE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS transactions_user_date_idx ON transactions (user_id, executed_on, id);
