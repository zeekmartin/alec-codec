FROM python:3.11-slim

WORKDIR /app

# Install dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application
COPY main.py .

# Environment
ENV PYTHONUNBUFFERED=1
ENV PORT=8082
ENV SIMULATOR_URL=http://simulator:8080
ENV POLL_INTERVAL=1.0
ENV WINDOW_SIZE=100

EXPOSE 8082

HEALTHCHECK --interval=10s --timeout=5s --start-period=5s --retries=3 \
    CMD python -c "import httpx; httpx.get('http://localhost:8082/health').raise_for_status()"

CMD ["python", "main.py"]
