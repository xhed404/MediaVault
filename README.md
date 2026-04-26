# MediaVault

Self-hosted домашний архив файлов/медиа: загрузка через браузер, поиск, альбомы, дубликаты, шаринг ссылкой.

## Быстрый старт (Docker)

1. Создай `.env` из примера:

   - [`.env.example`](file:///workspace/.env.example)

2. Запусти:

```bash
docker compose up --build
```

Открой: http://localhost:8080

## Первый админ

Админ создаётся на старте через env:

- `BOOTSTRAP_ADMIN_EMAIL`
- `BOOTSTRAP_ADMIN_PASSWORD`

В Docker это можно добавить в `docker-compose.yml` (environment).

## Локальная разработка

Backend:

```bash
export SESSION_SECRET="change_me_to_a_long_random_string"
export DATABASE_URL="sqlite:./app.sqlite"
export STORAGE_ROOT="./data"
export WEB_DIST="/workspace/apps/web/dist"
cargo run -p mediavault_server
```

Frontend:

```bash
cd apps/web
npm install
npm run dev
```

## Структура

- [`apps/server`](file:///workspace/apps/server) — Rust backend (Axum + SQLite)
- [`apps/web`](file:///workspace/apps/web) — React/Vite web UI
- [`docker-compose.yml`](file:///workspace/docker-compose.yml) — self-hosted запуск

