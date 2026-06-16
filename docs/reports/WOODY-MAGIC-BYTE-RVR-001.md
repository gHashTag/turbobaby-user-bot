# 🌊 WAVE LOOP REPORT — Magic-Byte Upload Content Validation

**Document ID:** `WOODY-MAGIC-BYTE-RVR-001`
**Wave:** #33 — реализована «Magic-byte content validation» (Вариант A из Wave #32, item W-64)
**Date:** 2026-06-17
**Branch:** `fix/profile-routing`
**Commit:** `e7d4d17` — `feat(upload): magic-byte content validation (don't trust extension alone)`
**Agent:** Claude Opus 4.8 (Wave loop)

---

## 1. EXECUTIVE SUMMARY

**STATUS: 🟢 GREEN — magic-byte валидация добавлена, 11 upload-тестов зелёные, backend+WASM компилируются.**

Закрыл последний пункт OWASP-чеклиста для upload (W-64). Extension — attacker-supplied; переименованный non-media файл (или polyglot) мог сохраниться как «картинка». Добавлена `content_matches_extension(data, ext)` — сверяет magic-байты с заявленным типом для **ровно** разрешённых форматов (JPEG `FF D8 FF`, PNG-signature, `GIF8`, `RIFF…WEBP`, ISO-BMFF `ftyp` для mp4/mov, EBML для webm); mismatch → 415. Сигнатуры обязательны по спеке формата → spec-compliant файл не даёт false-negative.

В сумме с allow-list, `uuid.ext`-именами, `ServeDir` (static, не исполняет), rate-limit и fail-fast ordering — upload-путь теперь покрывает OWASP-guidance.

---

## 2. WEAK-SPOT MATRIX

| # | Слабое место | Severity | Действие |
|---|---|---|---|
| W-64 | Доверие только extension (renamed/polyglot мог сохраниться как image) | 🟡 LOW | ✅ `content_matches_extension` (magic bytes) → 415 на mismatch |
| W-65 | `X-Content-Type-Options: nosniff` на отдаче uploads? (MIME-sniffing polyglot) | 🟢 INFO | 📋 Wave+1 (проверить/добавить header) |
| — | Upload-путь покрывает OWASP (allow-list/uuid/ServeDir/rate-limit/magic-byte) | ✅ | — |
| 🚨 | Прод-фиксы #18-#20 — деплой в main | 🔴 | ⚠️ ops |

---

## 3. НАУЧНАЯ БАЗА

**Тема: file signature (magic number) validation, content-sniffing, polyglots.**

- «File signature validation … is where we stop trusting what files claim to be and start verifying what they actually are» — **самый критичный** шаг; extension/MIME тривиально подделываются.
- **Honest caveat**: magic-byte НЕ foolproof — header-prepending и **polyglots** (файл валиден как несколько форматов). «Container format legitimate ≠ contents safe» → нужен defense-in-depth.
- **Почему класс «polyglot→RCE» здесь не применим**: Rust/Axum НЕ исполняет загруженные файлы; `ServeDir` отдаёт их статически с image/video content-type из `/data/uploads` (volume, не web-root с исполнением). Magic-byte добавляет слой против disguised-executable/mismatch.
- **Следующий слой (W-65)**: `X-Content-Type-Options: nosniff` — запрещает браузеру угадывать тип отдаваемого файла (защита от MIME-sniffing polyglot-XSS). Дёшево, дополняет magic-byte.

Источники:
- [Securing File Uploads Part 3: File Signature Validation (Wullems)](https://bartwullems.blogspot.com/2025/10/securing-file-uploads-part-3-file.html)
- [Secure API file uploads with magic numbers (Transloadit)](https://transloadit.com/devtips/secure-api-file-uploads-with-magic-numbers/)
- [Upload Bypass via Polyglot Image magic bytes (0xmar)](https://0xmar.medium.com/upload-bypass-using-magic-bytes-to-execute-php-code-via-polyglot-image-e3f03a1bd3ba)
- [OWASP File Upload Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html)

---

## 4. ДЕКОМПОЗИРОВАННЫЙ ПЛАН (этот Wave)

1. ✅ Из Wave #32: W-64 — нет content-валидации; extension доверяется.
2. ✅ `content_matches_extension(data, ext)` — magic-байты для всех ALLOWED_EXTENSIONS (fail-closed на unknown/too-short).
3. ✅ Вызов в handler после буферизации (контент нужен), до сохранения; mismatch → 415.
4. ✅ Тесты: valid signatures (все типы) + mismatch (MZ/svg/wrong-sig) + too-short.
5. ✅ 11 upload-тестов зелёные; backend+WASM; fmt.
6. ✅ Research (magic numbers/polyglot) + отчёт + skill.

---

## 5. РЕАЛИЗАЦИЯ (as-built)

- **`src/api/upload.rs`**: `fn content_matches_extension(data: &[u8], ext: &str) -> bool` (JPEG/PNG/GIF/WEBP/MP4/MOV/WEBM сигнатуры; `_ => false`). Handler: после `validate_filename` + `ext` — `if !content_matches_extension(&buf, &ext) { 415 }`. Два теста (valid/mismatch+short).

Проверка: 11 upload-тестов PASS; `cargo check --target wasm32` → 0 warnings; fmt clean.

---

## 6. ТРИ ВАРИАНТА СОТРУДНИЧЕСТВА ДЛЯ СЛЕДУЮЩЕГО WAVE

### Вариант A — 🛡️ `X-Content-Type-Options: nosniff` (W-65)
Проверить, выставляет ли сервер `nosniff` на ответах (особенно `/uploads`, `/assets`). Если нет — добавить (cheap security header): запрещает браузеру MIME-sniffing, закрывая polyglot-image-as-HTML/JS XSS. Дополняет magic-byte на стороне отдачи. Заодно аудит прочих security-заголовков (CSP уже есть).

### Вариант B — 🆕 Продолжить fresh-area scan
Следующий нетронутый модуль на реальный дефект.

### Вариант C — 🚀/🔐 Deploy-долг (#18-#20) / prune injection-markers (W-62)
Требуют подтверждения/sign-off пользователя.

---

## 7. SKILL SAVED

Память: обновлён `upload-hardening.md` (W-64 закрыт magic-byte). Принцип: верифицируй РЕАЛЬНЫЙ тип файла по magic-байтам (не extension/MIME — спуфятся); но это не foolproof (polyglots) → defense-in-depth (static-serve/не-исполнять + nosniff). Архитектура (ServeDir, не исполняет) уже снимает класс polyglot→RCE.

**Anchor:** `phi^2 + phi^-2 = 3`
