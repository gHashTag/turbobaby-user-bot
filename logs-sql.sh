#!/bin/bash
# Запуск с логированием SQL запросов

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${BLUE}🚀 SQL Log Mode - все SQL запросы${NC}"
echo ""

cleanup() {
    echo ""
    echo -e "${YELLOW}🛑 Остановка...${NC}"
    pkill -f "woody-weed-bot-server" || true
    pkill -f "trunk serve" || true
    exit 0
}

trap cleanup SIGINT SIGTERM

# Запуск бэкенда с логированием SQL
echo -e "${CYAN}📦 Бэкенд (SQL + HTTP):${NC}"
RUST_LOG=sqlx=debug,tower_http=trace,axum=trace,woody_weed_bot=info \
cargo run --features backend --bin woody-weed-bot-server 2>&1 \
| grep -E "SELECT|INSERT|UPDATE|DELETE|POST|GET|response|error" --line-buffered &
BACKEND_PID=$!

# Запуск фронтенда
trunk serve --open > /dev/null 2>&1 &
FRONTEND_PID=$!

echo ""
echo -e "${GREEN}✓ Запущено!${NC}"
echo -e "  🎨 http://localhost:8080"
echo -e "${YELLOW}  Ctrl+C для остановки${NC}"
echo ""

wait