@REPO_NAME@ - Docker deployment bundle (Linux)
================================================

【Contents】
  images/backend.tar.gz     Backend image (gzip archive; includes SPA static files)
  docker-compose.yml        Compose stack (backend + bundled Redis)
  config.toml.example       Backend configuration template
  load-and-start.sh         Load images and start services
  stop.sh                   Stop services
  README.txt
  VERSION

【System requirements】
  - Linux x86_64
  - Docker 24+ with docker compose v2 plugin
  - External database (MySQL / Postgres / SQLite); not included in this package
  - Redis is bundled in the compose file (used as cache; no external Redis required)

【Install and start】
  1. cp config.toml.example config.toml
  2. Edit config.toml:
       - Set the external database URL
       - Set JWT secret and encryption key
       - Keep [server].static_dir = "web" (backend serves the SPA)
       - [redis].url should be "redis" (the bundled service name)
  3. ./load-and-start.sh

  The backend listens on port 3000 inside the container. /app/web holds the SPA
  static files, which the backend serves from the same origin.
  The database URL in config.toml must be reachable from the container
  (use the host IP or an internal network IP, not 127.0.0.1 / localhost).
  Redis is on the compose internal network, so [redis].url is simply "redis".

【Access】
  Open http://<host>:3000 in a browser   # serves both SPA and /api/*

【Logs】
  docker compose logs -f backend

【Stop】
  ./stop.sh                           # docker compose down; containers are removed
  Data remains in ./data/ and ./logs/

【Full cleanup】
  ./stop.sh
  docker rmi @REPO_NAME@:<VERSION>
  rm -rf data logs config.toml
