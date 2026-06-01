# Smoke tests — order endpoint defences

Manual curl-based smoke for the three defences shipped in cycles #56 — #58:

* **Price authority** across every catalog (cycle #58 / C)
* **Idempotency** key replay (cycle #57)
* **TTL sweep** for stale idempotency keys (cycle #58 / A)

These tests assume a local backend on `http://localhost:8080` and a real
strain/accessory/tea/set in the DB. Replace IDs and `init_data` for your
environment.

> **Setup**
>
> ```sh
> cargo run --features backend &
> # Pick a strain id and the real price from the menu:
> STRAIN_ID=$(curl -s http://localhost:8080/api/strains | jq -r '.strains[0].id')
> REAL_PRICE=$(curl -s http://localhost:8080/api/strains | jq -r '.strains[0].price_per_gram')
> # Telegram init_data — use whatever your bot setup produces. For local
> # testing without a real Telegram WebApp, the auth check accepts an empty
> # string when telegram_id is omitted (anonymous order).
> INIT=""
> ```

## 1. Happy path — order at real price

Should return `200 OK` with a fresh `order_id`.

```sh
curl -s -X POST http://localhost:8080/api/orders \
  -H 'Content-Type: application/json' \
  -H "X-Telegram-Init-Data: $INIT" \
  -d '{
    "items": [{"strain_id":"'"$STRAIN_ID"'","strain_name":"x","quantity":2}],
    "subtotal": '"$(echo "$REAL_PRICE * 2" | bc -l)"',
    "total": '"$(echo "$REAL_PRICE * 2" | bc -l)"'
  }'
# expected: {"success":true,"order_id":"<uuid>"}
```

## 2. Price tampering — server-side authority rejects

Claim 1 baht for premium strain — server computes real price and returns 422.

```sh
curl -is -X POST http://localhost:8080/api/orders \
  -H 'Content-Type: application/json' \
  -H "X-Telegram-Init-Data: $INIT" \
  -d '{
    "items": [{"strain_id":"'"$STRAIN_ID"'","strain_name":"x","quantity":5}],
    "subtotal": 1.0,
    "total": 1.0
  }' | head -1
# expected: HTTP/1.1 422 Unprocessable Entity
# server log: warn! "create_order: subtotal mismatch — possible client tampering"
```

## 3. Unknown catalog id — short-circuit

Reject 422 with `unknown_item` reason, no DB INSERT happens.

```sh
curl -is -X POST http://localhost:8080/api/orders \
  -H 'Content-Type: application/json' \
  -H "X-Telegram-Init-Data: $INIT" \
  -d '{
    "items": [{"set_id":"00000000-0000-0000-0000-000000000000","set_name":"x","quantity":1}],
    "subtotal": 0,
    "total": 0
  }' | head -1
# expected: HTTP/1.1 422 Unprocessable Entity
# server log: warn! "create_order: order references missing catalog item"
```

## 4. Idempotency replay — same key returns same order_id

Two POSTs with identical `X-Idempotency-Key`. First creates the order;
second returns it back with `idempotent_replay: true` instead of inserting
a duplicate.

```sh
KEY=$(uuidgen)
ORDER1=$(curl -s -X POST http://localhost:8080/api/orders \
  -H 'Content-Type: application/json' \
  -H "X-Telegram-Init-Data: $INIT" \
  -H "X-Idempotency-Key: $KEY" \
  -d '{
    "items":[{"strain_id":"'"$STRAIN_ID"'","strain_name":"x","quantity":1}],
    "subtotal":'"$REAL_PRICE"',
    "total":'"$REAL_PRICE"'
  }' | jq -r .order_id)

# Wait > 1 minute to clear the per-user rate limit, then retry with the same key.
sleep 65
ORDER2=$(curl -s -X POST http://localhost:8080/api/orders \
  -H 'Content-Type: application/json' \
  -H "X-Telegram-Init-Data: $INIT" \
  -H "X-Idempotency-Key: $KEY" \
  -d '{
    "items":[{"strain_id":"'"$STRAIN_ID"'","strain_name":"x","quantity":1}],
    "subtotal":'"$REAL_PRICE"',
    "total":'"$REAL_PRICE"'
  }')

# expected: $ORDER1 == $(echo $ORDER2 | jq -r .order_id)
# expected: $(echo $ORDER2 | jq -r .idempotent_replay) == "true"
echo "Original: $ORDER1"
echo "Replay:   $ORDER2"
```

## 5. Malformed idempotency key — 400

Headers carrying control chars or oversize payloads bounce with 400 before
any DB work.

```sh
curl -is -X POST http://localhost:8080/api/orders \
  -H 'Content-Type: application/json' \
  -H "X-Telegram-Init-Data: $INIT" \
  -H "X-Idempotency-Key: has space/slash" \
  -d '{"items":[],"subtotal":0,"total":0}' | head -1
# expected: HTTP/1.1 400 Bad Request
```

## 6. TTL sweep — verify it runs

The sweep fires hourly on a background `tokio::spawn`. To verify it works
without waiting a full hour, insert a key with `created_at` >24h in the
past and watch the log for the next sweep tick (or trigger manually via
`psql`):

```sql
INSERT INTO order_idempotency_keys (key, order_id, telegram_id, created_at)
VALUES ('test-stale', '00000000-0000-0000-0000-000000000000', NULL,
        NOW() - INTERVAL '25 hours');

-- Wait for the next sweep tick (≤60 min), then:
SELECT * FROM order_idempotency_keys WHERE key = 'test-stale';
-- expected: 0 rows (deleted by the sweep)
-- log: info! "idempotency_keys: TTL sweep removed expired rows" deleted=1
```

For instant verification you can call the helper directly from a `cargo
test` runner that has a live DB pool, or shorten the retention to 1 hour
in a fork of `main.rs` for a one-off run.

## Checklist

- [ ] §1 happy path returns `order_id`
- [ ] §2 tampered subtotal → 422 + audit-log warn line
- [ ] §3 unknown set_id → 422 + audit-log warn line
- [ ] §4 idempotent replay returns same `order_id` + `idempotent_replay: true`
- [ ] §5 malformed idempotency key → 400 (before any DB work)
- [ ] §6 stale `order_idempotency_keys` row deleted by sweep
