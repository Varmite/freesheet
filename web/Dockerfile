# FreeSheet web edition in Docker (optional; install.sh is the simpler route).
# Built from the repository root, because it needs src/, content/ and audio/ too:
#   cd web && docker compose up -d --build
# (docker-compose.yml sets the context; Dockerfile.dockerignore keeps the build small.)
FROM node:22-slim

WORKDIR /app
COPY package.json ./package.json
COPY src ./src
COPY content ./content
COPY audio ./audio

WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --omit=dev --no-audit --no-fund
COPY web ./

ENV NODE_ENV=production DATA_DIR=/data PORT=5000 AUTO_UPDATE=0
RUN mkdir -p /data && chown node:node /data
VOLUME /data
EXPOSE 5000
USER node
CMD ["node", "server.js"]
