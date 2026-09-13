# Woody Weed Bot - удобные команды

.PHONY: help dev backend frontend logs logs-logs-api sql-logs clean

.DEFAULT_GOAL := help

help: ## Показать эту справку
	@echo "🌿 Woody Weed Bot - команды:"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[0;32m%-20s\033[0m %s\n", $$1, $$2}'

dev: ## Dev режим - запуск бэкенда + фронтенда с DEBUG логами
	@./dev.sh

backend: ## Запустить только бэкенд с DEBUG логами
	@echo "📦 Бэкенд с DEBUG логами:"
	@PORT=3000 RUST_LOG=debug,sqlx=debug,tower_http=debug,axum=debug cargo run --features backend --bin turbobaby-bot-server

backend-quiet: ## Запустить только бэкенд без логов
	@echo "📦 Бэкенд (тихий режим):"
	@PORT=3000 cargo run --features backend --bin turbobaby-bot-server > /dev/null 2>&1 &

frontend: ## Запустить только фронтенд (trunk serve)
	@echo "🎨 Trunk serve:"
	@trunk serve --open

build: ## Собрать всё для продакшена
	@echo "📦 Сборка WASM..."
	@trunk build --release
	@echo "✓ Сборка завершена: dist/"

check: ## Проверить код
	@cargo check --all-features

test: ## Запустить тесты
	@cargo test

clean: ## Очистить артефакты
	@cargo clean
	@rm -rf dist/
	@rm -rf logs/

logs: ## Показать логи фоновых процессов
	@echo "📊 Логи бэкенда:"
	@if [ -f logs/backend.log ]; then tail -f logs/backend.log; else echo "Нет логов бэкенда"; fi

logs-frontend: ## Логи только фронтенда
	@echo "📊 Логи фронтенда:"
	@if [ -f logs/frontend.log ]; then tail -f logs/frontend.log; else echo "Нет логов фронтенда"; fi

logs-api: ## Запустить с фильтрацией только HTTP запросов
	@echo "📊 HTTP запросы:"
	@PORT=3000 RUST_LOG=tower_http=trace,axum=trace,sqlx=query,warn,error,teloxide=off,tracing=off \
	    cargo run --features backend --bin turbobaby-bot-server 2>&1 \
	    | grep -E "GET|POST|PUT|DELETE|PATCH|response|request" --line-buffered

sql-logs: ## Запустить с логированием SQL запросов
	@echo "📊 SQL + HTTP запросы:"
	@PORT=3000 RUST_LOG=sqlx=debug,tower_http=trace,axum=trace,turbobaby_bot=info \
	    cargo run --features backend --bin turbobaby-bot-server 2>&1 \
	    | grep -E "SELECT|INSERT|UPDATE|DELETE|POST|GET|response|request|error" --line-buffered

restart: ## Перезапустить (остановить и запустить)
	@pkill -f "turbobaby-bot-server" 2>/dev/null || true
	@pkill -f "trunk" 2>/dev/null || true
	@echo "✓ Остановлено"
	@./dev.sh
