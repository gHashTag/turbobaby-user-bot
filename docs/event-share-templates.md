# Share templates for Telegram posts

Use these snippets when posting events or products to the bot channel / broadcast list.
The deep links survive native Telegram forwarding.

## Event

```
🗓 {EVENT_TITLE}
📍 {LOCATION}
🕒 {DATE_TIME_BANGKOK}

👉 https://t.me/{BOT_USERNAME}?startapp=p_event_{EVENT_ID}
```

Inline button:
- Text: `Open event`
- URL: `https://t.me/{BOT_USERNAME}?startapp=p_event_{EVENT_ID}`

## Product (strain / accessory / tea / set)

```
🔥 {PRODUCT_NAME}
💰 {PRICE} ฿

👉 https://t.me/{BOT_USERNAME}?startapp=p_{KIND}_{PRODUCT_ID}
```

Kind prefixes:
- strain → `p_strain_{id}`
- accessory → `p_acc_{id}`
- tea → `p_tea_{id}`
- set → `p_set_{id}`

Inline button:
- Text: `Open`
- URL: `https://t.me/{BOT_USERNAME}?startapp=p_{KIND}_{PRODUCT_ID}`
