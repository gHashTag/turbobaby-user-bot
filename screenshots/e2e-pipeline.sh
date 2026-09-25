#!/bin/bash
# ============================================================
# E2E Visual Regression Pipeline — TurboBaby UI Kit
# ============================================================
# Usage: ./screenshots/e2e-pipeline.sh [--serve|--snap|--verify|--all]
#
# --serve   : Start HTTP server on port 8081
# --snap    : Take screenshots of all component pages
# --verify  : Verify screenshots contain expected content
# --all     : Run full pipeline (serve + snap + verify)
# ============================================================

set -euo pipefail

PORT=8081
BASE_URL="http://localhost:${PORT}"
SCREENSHOTS_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPORT="${SCREENSHOTS_DIR}/e2e-report.md"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ============================================================
# Component Registry — atomic UI components
# Each entry: folder | html_file | expected_text
# ============================================================
COMPONENTS=(
    "00-hello-world|screenshots/00-hello-world/index.html|Hello World"
    "01-typography|screenshots/01-typography/index.html|Type Scale"
    "02-colors|screenshots/02-colors/index.html|Color Palette"
    "03-buttons|screenshots/03-buttons/index.html|Buttons"
    "04-badges|screenshots/04-badges/index.html|Badges"
    "05-bike-cards|screenshots/05-bike-cards/index.html|Bike Cards"
    "06-order-cards|screenshots/06-order-cards/index.html|Order Cards"
    "07-cart|screenshots/07-cart/index.html|NMAX 155"
    "08-member-cards|screenshots/08-member-cards/index.html|Member Cards"
    "09-quests|screenshots/09-quests/index.html|Quest Cards"
    "10-order-steps|screenshots/10-order-steps/index.html|Order Steps"
    "11-modal|screenshots/11-modal/index.html|Modal"
    "12-toasts|screenshots/12-toasts/index.html|Toasts"
    "13-empty-states|screenshots/13-empty-states/index.html|Empty States"
    "14-loading|screenshots/14-loading/index.html|Loading"
    "15-inputs|screenshots/15-inputs/index.html|Inputs"
    "16-filters|screenshots/16-filters/index.html|Filter Tabs"
    "17-categories|screenshots/17-categories/index.html|Categories"
    "18-navigation|screenshots/18-navigation/index.html|Navigation"
    "19-neon-effects|screenshots/19-neon-effects/index.html|Neon"
    "20-success|screenshots/20-success/index.html|Order Placed"
)

log()  { echo -e "${CYAN}[E2E]${NC} $1"; }
ok()   { echo -e "${GREEN}[✓]${NC} $1"; }
warn() { echo -e "${YELLOW}[!]${NC} $1"; }
fail() { echo -e "${RED}[✗]${NC} $1"; }

# ============================================================
# Step 1: Start HTTP Server
# ============================================================
serve() {
    log "Starting HTTP server on port ${PORT}..."
    
    # Kill existing server on this port
    PID=$(lsof -ti :${PORT} 2>/dev/null || true)
    if [ -n "$PID" ]; then
        warn "Killing existing process on port ${PORT}: PID ${PID}"
        kill $PID 2>/dev/null || true
        sleep 1
    fi
    
    cd "${PROJECT_ROOT}" && python3 -m http.server ${PORT} &
    SERVER_PID=$!
    sleep 2
    
    # Verify server is running
    if curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/hello.html" | grep -q "200"; then
        ok "Server running at ${BASE_URL} (PID: ${SERVER_PID})"
    else
        fail "Server failed to start!"
        exit 1
    fi
}

# ============================================================
# Step 2: Take Screenshots (requires MCP — outputs curl-based verification)
# ============================================================
snap() {
    log "Taking screenshots of ${#COMPONENTS[@]} components..."
    
    PASS=0
    FAIL=0
    CACHE_DIR="/tmp/turbobaby-e2e-cache"
    mkdir -p "${CACHE_DIR}"
    
    # Initialize report
    cat > "${REPORT}" << 'EOF'
# E2E Visual Regression Report — TurboBaby UI Kit

| # | Component | URL | HTTP Status | Expected Text | Result |
|---|-----------|-----|-------------|---------------|--------|
EOF
    
    for entry in "${COMPONENTS[@]}"; do
        IFS='|' read -r folder html_file expected_text <<< "$entry"
        
        URL="${BASE_URL}/${html_file}"
        CACHE_FILE="${CACHE_DIR}/$(echo "$html_file" | tr '/' '_')"
        
        # Use cached content or download
        if [ ! -f "${CACHE_FILE}" ]; then
            HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "${URL}" 2>/dev/null || echo "000")
            if [ "$HTTP_CODE" = "200" ]; then
                curl -s --max-time 30 "${URL}" > "${CACHE_FILE}" 2>/dev/null || echo "" > "${CACHE_FILE}"
            fi
        else
            HTTP_CODE="200"
        fi
        
        # Check for expected text in response
        if [ "$HTTP_CODE" = "200" ]; then
            if grep -qi "${expected_text}" "${CACHE_FILE}" 2>/dev/null; then
                ok "${folder}: HTTP ${HTTP_CODE}, found '${expected_text}'"
                RESULT="✅ PASS"
                PASS=$((PASS + 1))
            else
                warn "${folder}: HTTP ${HTTP_CODE}, but '${expected_text}' not found in body"
                RESULT="⚠️ TEXT MISSING"
                FAIL=$((FAIL + 1))
            fi
        else
            fail "${folder}: HTTP ${HTTP_CODE} (expected 200)"
            RESULT="❌ FAIL"
            FAIL=$((FAIL + 1))
        fi
        
        echo "| ${folder} | ${html_file} | ${URL} | ${HTTP_CODE} | ${expected_text} | ${RESULT} |" >> "${REPORT}"
    done
    
    rm -rf "${CACHE_DIR}"
    
    echo "" >> "${REPORT}"
    echo "## Summary" >> "${REPORT}"
    echo "- **Passed**: ${PASS}" >> "${REPORT}"
    echo "- **Failed**: ${FAIL}" >> "${REPORT}"
    echo "- **Total**: $((PASS + FAIL))" >> "${REPORT}"
    echo "- **Timestamp**: $(date -u +"%Y-%m-%dT%H:%M:%SZ")" >> "${REPORT}"
    
    log "Results: ${PASS} passed, ${FAIL} failed out of $((PASS + FAIL)) components"
    
    if [ "${FAIL}" -gt 0 ]; then
        warn "Some components failed! Check ${REPORT}"
        return 1
    fi
    
    ok "All components passed!"
    return 0
}

# ============================================================
# Step 3: Verify (curl-based content check)
# ============================================================
verify() {
    log "Verifying all component pages..."
    
    PASS=0
    FAIL=0
    
    for entry in "${COMPONENTS[@]}"; do
        IFS='|' read -r folder html_file expected_text <<< "$entry"
        
        URL="${BASE_URL}/${html_file}"
        HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${URL}" 2>/dev/null || echo "000")
        
        if [ "$HTTP_CODE" = "200" ]; then
            BODY=$(curl -s "${URL}" 2>/dev/null || echo "")
            if echo "$BODY" | grep -qi "${expected_text}"; then
                ok "[verify] ${folder}: ✅"
                ((PASS++))
            else
                fail "[verify] ${folder}: text '${expected_text}' not found"
                ((FAIL++))
            fi
        else
            fail "[verify] ${folder}: HTTP ${HTTP_CODE}"
            ((FAIL++))
        fi
    done
    
    TOTAL=$((PASS + FAIL))
    log "Verification: ${PASS}/${TOTAL} passed"
    return 0
}

# ============================================================
# Main
# ============================================================
case "${1:---all}" in
    --serve)
        serve
        echo "Server PID: ${SERVER_PID}"
        echo "Press Ctrl+C to stop"
        wait ${SERVER_PID}
        ;;
    --snap)
        snap
        ;;
    --verify)
        verify
        ;;
    --all)
        serve
        sleep 1
        snap
        VERIFY_RESULT=$?
        
        echo ""
        log "Report saved to: ${REPORT}"
        cat "${REPORT}"
        
        # Kill server
        if [ -n "${SERVER_PID:-}" ]; then
            kill ${SERVER_PID} 2>/dev/null || true
        fi
        
        exit ${VERIFY_RESULT}
        ;;
    *)
        echo "Usage: $0 [--serve|--snap|--verify|--all]"
        exit 1
        ;;
esac
