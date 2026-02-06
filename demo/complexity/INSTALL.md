# Instructions pour ajouter le service Complexity

## 1. Copier le dossier complexity dans le projet demo

```bash
# Depuis le dossier demo/
cp -r [path-to-complexity] ./complexity/
```

## 2. Ajouter au docker-compose.yml

Ajoute ce bloc dans la section `services:` :

```yaml
  complexity:
    build: ./complexity
    container_name: alec-complexity
    ports:
      - "8082:8082"
    environment:
      - SIMULATOR_URL=http://simulator:8080
      - POLL_INTERVAL=1.0
      - WINDOW_SIZE=100
    depends_on:
      simulator:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "python", "-c", "import httpx; httpx.get('http://localhost:8082/health').raise_for_status()"]
      interval: 10s
      timeout: 5s
      retries: 3
      start_period: 10s
    networks:
      - alec-network
    restart: unless-stopped
```

## 3. Ajouter à la config Prometheus

Dans `prometheus/prometheus.yml`, ajouter dans `scrape_configs:` :

```yaml
  - job_name: 'complexity'
    static_configs:
      - targets: ['complexity:8082']
    scrape_interval: 5s
```

## 4. Redémarrer la stack

```bash
docker compose down
docker compose build complexity
docker compose up -d
```

## 5. Vérifier

```bash
# Service running
curl http://localhost:8082/health

# Metrics exposées
curl http://localhost:8082/metrics | grep alec_

# Status détaillé
curl http://localhost:8082/status
```

## Métriques exposées

| Métrique | Description |
|----------|-------------|
| `alec_entropy_sensor{sensor_id}` | Entropie par capteur (H_i) |
| `alec_entropy_total` | Entropie totale (H_tot) |
| `alec_complexity` | Complexité (C) |
| `alec_robustness` | Robustesse (R) 0-1 |
| `alec_information_total` | Information totale (I_tot) |
| `alec_delta_information` | Variation I_tot (ΔI_tot) |
| `alec_correlation_mean` | Corrélation moyenne |
| `alec_anomaly_detected{sensor_id}` | Anomalie détectée |
| `alec_anomaly_count` | Nombre d'anomalies |

## Endpoints API

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Health check |
| `GET /metrics` | Prometheus metrics |
| `GET /status` | JSON avec toutes les métriques |
| `POST /reset` | Reset le calculateur |
