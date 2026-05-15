#!/bin/bash
# Запуск с фильтрацией только HTTP запросов

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${BLUE}🚀 API Log Mode - только HTTP запросы${NC}"
echo ""

cleanup() {
    echo ""
    echo -e "${YELLOW}🛑 Остановка...${NC}"
    pkill -f "woody-weed-bot-server" || true
    pkill -f "trunk serve" || true
    exit 0
}

trap cleanup SIGINT SIGTERM

# Запуск бэкенда с фильтрацией только HTTP
echo -e "${CYAN}📦 Бэкенд (только HTTP запросы):${NC}"
RUST_LOG=woody_weed_bot=debug,tower_http=trace,axum=trace,\
sqlx=query,warn,error,teloxide=off,tracing=off \
cargo run --features backend --bin woody-weed-bot-server 2>&1 \
| grep -E "GET|POST|PUT|DELETE|PATCH|response|request" --line-buffered &
BACKEND_PID=$!

# Запуск фронтенда в тишине
trunk serve --open > /dev/null 2>&1 &
FRONTEND_PID=$!

echo ""
echo -e "${GREEN}✓ Запущено!${NC}"
echo -e "  🎨 http://localhost:8080"
echo -e "${YELLOW}  Ctrl+C для остановки${NC}"
echo ""

wait