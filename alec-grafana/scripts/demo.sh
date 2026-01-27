#!/bin/bash
# ALEC Grafana Demo Script
# Starts the complete monitoring stack with sample data

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}"
echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║                     ALEC Monitoring Demo                      ║"
echo "║         Adaptive Lossless Entropy Codec - Grafana Stack       ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

# Check prerequisites
check_prereqs() {
    echo -e "${YELLOW}Checking prerequisites...${NC}"

    if ! command -v docker &> /dev/null; then
        echo -e "${RED}Error: Docker is not installed${NC}"
        exit 1
    fi

    if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
        echo -e "${RED}Error: Docker Compose is not installed${NC}"
        exit 1
    fi

    echo -e "${GREEN}Prerequisites OK${NC}"
}

# Start the stack
start_stack() {
    echo -e "${YELLOW}Starting ALEC monitoring stack...${NC}"

    cd "$PROJECT_DIR"

    # Use docker compose (new) or docker-compose (old)
    if docker compose version &> /dev/null; then
        docker compose up -d --build
    else
        docker-compose up -d --build
    fi

    echo -e "${GREEN}Stack started${NC}"
}

# Stop the stack
stop_stack() {
    echo -e "${YELLOW}Stopping ALEC monitoring stack...${NC}"

    cd "$PROJECT_DIR"

    if docker compose version &> /dev/null; then
        docker compose down
    else
        docker-compose down
    fi

    echo -e "${GREEN}Stack stopped${NC}"
}

# Clean up (including volumes)
clean_stack() {
    echo -e "${YELLOW}Cleaning up ALEC monitoring stack...${NC}"

    cd "$PROJECT_DIR"

    if docker compose version &> /dev/null; then
        docker compose down -v --rmi local
    else
        docker-compose down -v --rmi local
    fi

    echo -e "${GREEN}Stack cleaned${NC}"
}

# Show status
show_status() {
    echo -e "${YELLOW}Stack status:${NC}"

    cd "$PROJECT_DIR"

    if docker compose version &> /dev/null; then
        docker compose ps
    else
        docker-compose ps
    fi
}

# Show logs
show_logs() {
    cd "$PROJECT_DIR"

    if docker compose version &> /dev/null; then
        docker compose logs -f "$@"
    else
        docker-compose logs -f "$@"
    fi
}

# Wait for services
wait_for_services() {
    echo -e "${YELLOW}Waiting for services to be ready...${NC}"

    local max_attempts=30
    local attempt=1

    # Wait for Grafana
    echo -n "Waiting for Grafana"
    while [ $attempt -le $max_attempts ]; do
        if curl -s http://localhost:3000/api/health | grep -q "ok" 2>/dev/null; then
            echo -e " ${GREEN}OK${NC}"
            break
        fi
        echo -n "."
        sleep 2
        attempt=$((attempt + 1))
    done

    if [ $attempt -gt $max_attempts ]; then
        echo -e " ${RED}TIMEOUT${NC}"
    fi

    # Wait for Prometheus
    attempt=1
    echo -n "Waiting for Prometheus"
    while [ $attempt -le $max_attempts ]; do
        if curl -s http://localhost:9090/-/healthy 2>/dev/null | grep -q "Healthy"; then
            echo -e " ${GREEN}OK${NC}"
            break
        fi
        echo -n "."
        sleep 2
        attempt=$((attempt + 1))
    done

    if [ $attempt -gt $max_attempts ]; then
        echo -e " ${RED}TIMEOUT${NC}"
    fi

    # Wait for Exporter
    attempt=1
    echo -n "Waiting for ALEC Exporter"
    while [ $attempt -le $max_attempts ]; do
        if curl -s http://localhost:9100/health 2>/dev/null | grep -q "OK"; then
            echo -e " ${GREEN}OK${NC}"
            break
        fi
        echo -n "."
        sleep 2
        attempt=$((attempt + 1))
    done

    if [ $attempt -gt $max_attempts ]; then
        echo -e " ${RED}TIMEOUT${NC}"
    fi
}

# Print access info
print_access_info() {
    echo ""
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}                     Services are ready!                        ${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "  ${BLUE}Grafana:${NC}        http://localhost:3000"
    echo -e "                  Username: admin"
    echo -e "                  Password: admin"
    echo ""
    echo -e "  ${BLUE}Prometheus:${NC}     http://localhost:9090"
    echo ""
    echo -e "  ${BLUE}ALEC Exporter:${NC}  http://localhost:9100"
    echo -e "                  Metrics: http://localhost:9100/metrics"
    echo -e "                  Status:  http://localhost:9100/status"
    echo ""
    echo -e "${YELLOW}Dashboard:${NC} Navigate to Grafana > Dashboards > ALEC > ALEC Overview"
    echo ""
    echo -e "${YELLOW}The demo is replaying agricultural sensor data at 60x speed.${NC}"
    echo -e "${YELLOW}Watch the metrics update in real-time!${NC}"
    echo ""
}

# Main
case "${1:-start}" in
    start)
        check_prereqs
        start_stack
        wait_for_services
        print_access_info
        ;;
    stop)
        stop_stack
        ;;
    restart)
        stop_stack
        start_stack
        wait_for_services
        print_access_info
        ;;
    status)
        show_status
        ;;
    logs)
        shift
        show_logs "$@"
        ;;
    clean)
        clean_stack
        ;;
    help|--help|-h)
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  start    Start the monitoring stack (default)"
        echo "  stop     Stop the monitoring stack"
        echo "  restart  Restart the monitoring stack"
        echo "  status   Show stack status"
        echo "  logs     Show logs (optionally specify service: logs grafana)"
        echo "  clean    Stop and remove all containers, volumes, and images"
        echo "  help     Show this help message"
        ;;
    *)
        echo "Unknown command: $1"
        echo "Run '$0 help' for usage"
        exit 1
        ;;
esac
