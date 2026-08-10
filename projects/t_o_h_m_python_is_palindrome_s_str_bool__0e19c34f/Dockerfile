# filepath: Dockerfile
FROM python:3.11-slim@sha256:124d73aee6fbab06658af5fe54d559e708e4bad6e40b7045c481a4d351b7a9eb

WORKDIR /app

COPY requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt

COPY conftest.py palindrome.py ./
COPY tests/ ./tests/

RUN useradd -m appuser && chown -R appuser:appuser /app
USER appuser

CMD ["python", "-m", "pytest", "-q"]
