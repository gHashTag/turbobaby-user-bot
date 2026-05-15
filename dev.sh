#!/bin/bash
# Dev Mode - запуск бэкенда и фронтенда с логами в терминал

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${BLUE}🚀 Dev Mode - запуск с логами${NC}"
echo ""

# Очистка при выходе
cleanup() {
    echo ""
    echo -e "${YELLOW}🛑 Остановка всех сервисов...${NC}"
    if [[ -n "$BACKEND_PID" ]]; then
        kill $BACKEND_PID 2>/dev/null || true
        echo -e "${GREEN}✓ Бэкенд остановлен${NC}"
    fi
    if [[ -n "$TRUNK_PID" ]]; then
        kill $TRUNK_PID 2>/dev/null || true
        echo -e "${GREEN}✓ Trunk остановлен${NC}"
    fi
    exit 0
}

trap cleanup SIGINT SIGTERM

# Остановить существующие процессы
echo -e "${CYAN}🧹 Очистка портов...${NC}"
pkill -f "woody-weed-bot-server" 2>/dev/null || true
pkill -f "trunk" 2>/dev/null || true
sleep 2

# Создание директории для логов
mkdir -p logs

# Запуск бэкенда на порту 3000 с логированием
echo -e "${CYAN}📦 Бэкенд (порт 3000):${NC}"
PORT=3000 RUST_LOG=debug,sqlx=debug,tower_http=debug,axum=debug \
    cargo run --features backend --bin woody-weed-bot-server 2>&1 \
    | sed 's/^/[BACKEND] /' &
BACKEND_PID=$!

# Ожидание старта бэкенда (до 20 секунд)
echo -e "${YELLOW}⏳ Ожидание запуска бэкенда...${NC}"
for i in {1..20}; do
    if ! kill -0 $BACKEND_PID 2>/dev/null; then
        echo -e "${RED}❌ Бэкенд упал при запуске!${NC}"
        # Покажем логи
        sleep 2
        kill $BACKEND_PID 2>/dev/null || true
        exit 1
    fi
    # Проверим не появился ли HTTP сервер
    if lsof -ti:3000 -sTCP:LISTEN 2>/dev/null > /dev/null; then
        echo -e "${GREEN}✓ Бэкенд запущен!${NC}"
        break
    fi
    sleep 1
done

if ! kill -0 $BACKEND_PID 2>/dev/null; then
    echo -e "${RED}❌ Бэкенд не запустился за 20 секунд${NC}"
    exit 1
fi

# Запуск trunk serve на порту 8080
echo -e "${CYAN}🎨 Trunk (порт 8080):${NC}"
trunk serve 2>&1 | sed 's/^/[TRUNK] /' &
TRUNK_PID=$!

# Ожидание запуска trunk (до 15 секунд)
echo -e "${YELLOW}⏳ Ожидание запуска trunk...${NC}"
for i in {1..15}; do
    if ! kill -0 $TRUNK_PID 2>/dev/null; then
        echo -e "${RED}❌ Trunk упал при запуске!${NC}"
        kill $BACKEND_PID 2>/dev/null || true
        exit 1
    fi
    if lsof -ti:8080 -sTCP:LISTEN 2>/dev/null > /dev/null; then
        echo -e "${GREEN}✓ Trunk запущен!${NC}"
        break
    fi
    sleep 1
done

if ! kill -0 $TRUNK_PID 2>/dev/null; then
    echo -e "${RED}❌ Trunk не запустился за 15 секунд${NC}"
    kill $BACKEND_PID 2>/dev/null || true
    exit 1
fi

# Информация о запущенных сервисах
echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════${NC}"
echo -e "${GREEN}🌿 Woody Weed Bot запущен!${NC}"
echo ""
echo -e "  🎨 Фронтенд: ${BLUE}http://localhost:8080${NC}"
echo -e "  📊 API (Backend): ${BLUE}http://localhost:3000${NC}"
echo -e "  👨‍💻 Админка:  ${BLUE}http://localhost:8080/admin${NC}"
echo -e "  🤖 Telegram:  ${BLUE}http://localhost:3000${NC} (webhook)"
echo ""
echo -e "${YELLOW}Логи уровня DEBUG:${NC}"
echo -e "${YELLOW}  - HTTP запросы (tower_http, axum)${NC}"
echo -e "${YELLOW}  - SQL запросы (sqlx)${NC}"
echo -e "${YELLOW}  - Все отладочные сообщения${NC}"
echo ""
echo -e "${YELLOW}Нажмите Ctrl+C для остановки${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════${NC}"
echo ""

# Ждем Ctrl+C
wait
