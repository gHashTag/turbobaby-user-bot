# 🌊 WAVE LOOP REPORT — Add-to-Cart from the Detail Modal

**Document ID:** `WOODY-MODAL-ADDTOCART-RVR-001`
**Wave:** #11 — закрыт UX-недочёт детальной модалки (Wave #3)
**Date:** 2026-06-16
**Branch:** `fix/profile-routing`
**Commit:** `8f3e7bf` — `feat(catalog): add-to-cart directly from the product detail modal`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — кнопка добавлена во всех 4 каталогах, WASM без warning'ов, fitness-тест зелёный.**

Перед выбором задачи проверил «слабые места»: UI panic-чист (0 prod-path unwrap/expect/panic), backend-middleware тоже, а оставшиеся menu-опции оказались **малоценными** (таблица `sets` — admin-only, её не видят пользователи; 64 `allow(dead_code)` вне экранов — в основном намеренные SeaORM-entity). Поэтому выбрал **реальный user-facing недочёт**: `ProductDetailModal` (Wave #3) был **read-only** — чтобы купить, пользователь закрывал модалку и искал кнопку на карточке.

Добавлена доминирующая зелёная кнопка «В корзину 🛒» над «Закрыть»; по клику товар кладётся в корзину и модалка закрывается. Подключено во всех 4 каталогах (menu/accessories/tea/sets), переиспользуя существующую CartItem-логику каждого экрана. Sold-out/«цена по запросу» → `can_add: false` (кнопка скрыта).

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-29 | Детальная модалка read-only → лишний шаг до покупки | 🟠 MED | ✅ Кнопка «В корзину» в модалке (4 каталога) |
| — | UI/backend prod-path panics | — | ✅ Проверено: 0 (PANIC_AUDIT держит) |
| W-30 | `sets`-таблица admin-only (не видна пользователям) → локализация её низкоценна | 🟢 INFO | переоценено: Вариант A прошлого Wave обесценен |
| W-04 | Garden-seed: историческое обилие хотфиксов (миграции 026/036/037, force-seed), нет регресс-гварда | 🟠 MED | 📋 Wave+1 |
| W-03 | 221 трекаемый бинарник в git | 🟠 MED | 📋 нужно подтверждение |

---

## 3. НАУЧНАЯ / ИНДУСТРИАЛЬНАЯ БАЗА

**Тема: interaction cost и сокращение шагов в воронке PDP → cart.**

- **Interaction cost (HCI/NN-group-принцип)**: каждый лишний клик/прокрутка/закрытие повышает стоимость взаимодействия и роняет конверсию. Модалка-без-кнопки заставляла: закрыть → найти карточку → нажать. Кнопка в модалке убирает 2 шага в момент решения о покупке.
- **Cart abandonment 50–80%**; «complex flows → reduce steps» — прямое UX-лекарство. Видимая, контрастная, крупная Add-to-Cart CTA снижает когнитивную и физическую стоимость (особенно в thumb-zone мобильного Telegram Mini App).
- Связь с прошлым: id остаётся каноническим (surrogate key, server price-authority по id; Wave #6), локализованное имя — презентационный снимок и в карточке, и в модалке.

⚠️ Источники преимущественно индустриальные (vendor-гайды), конкретные цифры (напр. «−7% конверсии на +1с») — широко цитируемые отраслевые, а не рецензируемые. Принцип interaction-cost — устойчивый HCI-консенсус.

Источники:
- [eCommerce Friction: Guide to Common UX Issues (Mouseflow)](https://mouseflow.com/blog/ecommerce-friction/)
- [Add to Cart Conversion Rate (Fermat)](https://www.fermatcommerce.com/post/add-to-cart-conversion-rate)
- [Mobile Commerce Conversion Rate Optimization Guide (Storyly)](https://www.storyly.io/post/mobile-commerce-conversion-rate-optimization)
- [19 Ways to Improve Shopping Cart UX (BelVG)](https://belvg.com/blog/best-practices-for-ecommerce-shopping-carts.html)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Разведка: UI/backend panic-скан (чисто); переоценка pending-опций (sets admin-only, entity-allow намеренны) → выбран user-facing недочёт W-29.
2. ✅ `ProductDetailModal`: новые пропсы `on_add_to_cart: EventHandler<()>`, `can_add: bool`, `add_to_cart_label`; зелёная кнопка над «Закрыть».
3. ✅ 4 call-site: menu/accessories/tea/sets — передан CartItem-замыкание + доступность.
4. ✅ Borrow-after-move: значения для корзины переклонированы на месте модалки (card-замыкание уже забрало оригиналы).
5. ✅ `cargo check --target wasm32` → 0 warnings; `cargo fmt`; bilingual fitness-тест зелёный; commit.
6. ✅ Research + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`product_detail_modal.rs`**: `can_add` (default true), `add_to_cart_label: Option<String>` (default RU «В корзину 🛒»), `on_add_to_cart: EventHandler<()>`. Кнопка `#39ff14` над «Закрыть»; onclick → `stop_propagation` + `on_add_to_cart` + `on_close`.
- **menu/accessories/sets**: в `detail_open().then(|| { let add_id=…; let add_name=…; rsx!{…} })` — переклонированные id/name/price (Copy-цена), `can_add = доступность (& есть цена для strains)`, `CartItemType::{Strain,Accessory,Set}`.
- **tea**: screen-level модалка из `selected_tea` — id/name/price/доступность из `tea`, `CartItemType::Tea`.

Проверка: `cargo check --target wasm32-unknown-unknown` → 0 warnings; `every_bilingual_field_is_consumed` → green.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🔢 Quantity selector в модалке
Добавить выбор количества (−/+) в `ProductDetailModal` перед «В корзину», чтобы класть N грамм/штук за раз. Естественное продолжение; снижает повторные открытия. Frontend-only, переиспользует `CartItem.quantity`.

### Вариант B — 🌱 Garden-seed regression guard (W-04)
Историческая зона хотфиксов (миграции 026/036/037 + force-seed). Исследовать чисто-функциональные инварианты сидинга (`trios/garden.rs`) и закрыть host-тестом, чтобы прекратить серию реактивных правок. Высокая ценность; требует разведки тестируемости без БД.

### Вариант C — 🧹 Repo hygiene (W-03, нужно подтверждение)
Вынести `dist/.stage/`, `*.mp4`, аплоады из git (221 трекаемый бинарник). Требует подтверждения по деплою Railway/Trunk.

---

## 7. SKILL SAVED

Память: новый `detail-modal-addtocart.md` + строка в `MEMORY.md`. Зафиксировано: модалка теперь активна (add-to-cart, `can_add`), принцип interaction-cost (сокращай шаги до покупки), и переоценка — `sets`-таблица admin-only.

**Anchor:** `phi^2 + phi^-2 = 3`
