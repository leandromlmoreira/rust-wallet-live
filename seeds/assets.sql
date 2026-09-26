-- Ativos de exemplo para explorar o dashboard localmente.
-- psql postgres://postgres:postgres@localhost:5432/postgres -f seeds/assets.sql
INSERT INTO assets (name, unit_value) VALUES
 ('Bitcoin', 350000.00),
 ('Ethereum', 13500.00),
 ('PETR4', 38.40),
 ('ITUB4', 35.10),
 ('Tesouro Selic 2029', 15230.55)
ON CONFLICT (name) DO NOTHING;
