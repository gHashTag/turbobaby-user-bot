# Monitoring

## Prometheus

Configure a scrape target in `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'turbobaby-bot'
    scheme: 'https'
    metrics_path: '/metrics'
    static_configs:
      - targets: ['turbobaby-bot-production.up.railway.app:443']
    tls_config:
      insecure_skip_verify: false
```

The `/metrics` endpoint is served by `axum-prometheus` and exposes:
- `axum_http_requests_total` — request counter (labels: method, endpoint, status)
- `axum_http_requests_duration_seconds` — latency histogram
- `axum_http_requests_pending` — in-flight requests gauge

Custom counters (add to application code as needed):
- `orders_created_total`
- `quests_created_total` (label: kind)
- `users_registered_total`
- `qr_scans_total` (label: final)

## Grafana

### Import dashboard

1. Open Grafana → **Dashboards** → **New** → **Import**
2. Upload `grafana-dashboard.json` or paste its contents
3. Select your Prometheus datasource when prompted
4. Click **Import**

### Dashboard panels (8 total)

| # | Title | Type | Query |
|---|-------|------|-------|
| 1 | HTTP Request Rate | timeseries | `rate(axum_http_requests_total[5m])` |
| 2 | HTTP Error Rate (5xx) | timeseries | `rate(axum_http_requests_total{status=~"5.."}[5m])` |
| 3 | HTTP Latency P95 | timeseries | `histogram_quantile(0.95, rate(axum_http_requests_duration_seconds_bucket[5m]))` |
| 4 | Orders Created | timeseries | `rate(orders_created_total[5m])` |
| 5 | Quests Created | timeseries | `sum by (kind) (rate(quests_created_total[5m]))` |
| 6 | Users Registered | timeseries | `rate(users_registered_total[1h])` |
| 7 | QR Scans | timeseries | `sum by (final) (rate(qr_scans_total[5m]))` |
| 8 | Pending Requests | gauge | `axum_http_requests_pending` |

## Swagger UI

After deployment, the interactive API docs are available at:

```
https://turbobaby-bot-production.up.railway.app/swagger-ui/
```

The OpenAPI JSON spec is served at:

```
https://turbobaby-bot-production.up.railway.app/api-docs/openapi.json
```
