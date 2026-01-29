# ALEC Demo - Simulator Service
FROM python:3.11-slim

WORKDIR /app

# Install dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Install additional dependencies for the service
RUN pip install --no-cache-dir fastapi uvicorn[standard] prometheus-client

# Copy application
COPY . .

# Create data directory
RUN mkdir -p /app/data /app/profiles

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=10s --timeout=5s --start-period=10s --retries=3 \
    CMD python -c "import urllib.request; urllib.request.urlopen('http://localhost:8080/health')"

# Run
CMD ["python", "main.py"]
