#!/bin/bash
# Woody Weed Bot - запуск бэкенда и фронтенда одной командой

set -e

# Цвета для вывода
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}🌿 Woody Weed Bot - запуск всех сервисов${NC}"
echo ""

# Очистка при выходе
cleanup() {
    echo ""
    echo -e "${YELLOW}🛑 Остановка всех сервисов...${NC}"
    if [[ -n "$BACKEND_PID" ]]; then
        kill $BACKEND_PID 2>/dev/null || true
        echo -e "${GREEN}✓ Бэкенд остановлен${NC}"
    fi
    if [[ -n "$FRONTEND_PID" ]]; then
        kill $FRONTEND_PID 2>/dev/null || true
        echo -e "${GREEN}✓ Фронтенд остановлен${NC}"
    fi
    exit 0
}

trap cleanup SIGINT SIGTERM

# Проверка зависимостей
echo -e "${BLUE}📦 Проверка зависимостей...${NC}"

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ cargo не найден. Установите Rust: https://rustup.rs${NC}"
    exit 1
fi

if ! command -v trunk &> /dev/null; then
    echo -e "${RED}❌ trunk не найден. Установите: cargo install trunk${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Все зависимости найдены${NC}"
echo ""

# Создание директории для логов
mkdir -p logs

# Запуск бэкенда
echo -e "${BLUE}🚀 Запуск бэкенда...${NC}"
cargo run --features backend --bin turbobaby-bot-server > logs/backend.log 2>&1 &
BACKEND_PID=$!

# Ожидание запуска бэкенда
echo -e "${YELLOW}⏳ Ожидание запуска бэкенда (PID: $BACKEND_PID)...${NC}"
for i in {1..30}; do
    if kill -0 $BACKEND_PID 2>/dev/null; then
        sleep 1
        # Проверяем, не умер ли процесс с ошибкой
        if grep -q "ERROR" logs/backend.log 2>/dev/null; then
            echo -e "${RED}❌ Ошибка запуска бэкенда. Смотри logs/backend.log${NC}"
            cat logs/backend.log
            cleanup
        fi
        continue
    else
        echo -e "${RED}❌ Бэкенд упал при запуске${NC}"
        cat logs/backend.log
        cleanup
    fi
done

# Ждём немного, чтобы убедиться что бэкенд запущен стабильно
sleep 2

if ! kill -0 $BACKEND_PID 2>/dev/null; then
    echo -e "${RED}❌ Бэкенд не запустился${NC}"
    cat logs/backend.log
    cleanup
fi

echo -e "${GREEN}✓ Бэкенд запущен (PID: $BACKEND_PID)${NC}"
echo ""

# Запуск фронтенда
echo -e "${BLUE}🎨 Запуск фронтенда...${NC}"
trunk serve --open > logs/frontend.log 2>&1 &
FRONTEND_PID=$!

echo -e "${YELLOW}⏳ Ожидание запуска фронтенда (PID: $FRONTEND_PID)...${NC}"
sleep 3

if ! kill -0 $FRONTEND_PID 2>/dev/null; then
    echo -e "${RED}❌ Фронтенд не запустился${NC}"
    cat logs/frontend.log
    cleanup
fi

echo -e "${GREEN}✓ Фронтенд запущен (PID: $FRONTEND_PID)${NC}"
echo ""

# Информация о запущенных сервисах
echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
echo -e "${GREEN}🌿 Woody Weed Bot запущен!${NC}"
echo ""
echo -e "  📊 Бэкенд: ${BLUE}http://localhost:8080${NC}"
echo -e "  🎨 Фронтенд: ${BLUE}http://localhost:8080${NC}"
echo -e "  📋 Админка: ${BLUE}http://localhost:8080/admin${NC}"
echo ""
echo -e "${YELLOW}Логи:${NC}"
echo -e "  Бэкенд: ${BLUE}tail -f logs/backend.log${NC}"
echo -e "  Фронтенд: ${BLUE}tail -f logs/frontend.log${NC}"
echo ""
echo -e "${YELLOW}Нажмите Ctrl+C для остановки${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
echo ""

# Мониторинг процессов
while true; do
    if ! kill -0 $BACKEND_PID 2>/dev/null; then
        echo -e "${RED}❌ Бэкенд упал!${NC}"
        cat logs/backend.log
        cleanup
    fi
    if ! kill -0 $FRONTEND_PID 2>/dev/null; then
        echo -e "${RED}❌ Фронтенд упал!${NC}"
        cat logs/frontend.log
        cleanup
    fi
    sleep 5
done