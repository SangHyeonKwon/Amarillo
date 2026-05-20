# ── Build stage ────────────────────────────────────────────────
FROM node:22-alpine AS build
WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .

# Baked at build time — the dashboard runs in the browser, so this must be
# reachable from the browser, not a docker-internal hostname.
ARG VITE_API_BASE_URL=http://localhost:3000
ARG VITE_APP_BASE_PATH=/
ENV VITE_API_BASE_URL=$VITE_API_BASE_URL
ENV VITE_APP_BASE_PATH=$VITE_APP_BASE_PATH
RUN npm run build

# ── Runtime stage ──────────────────────────────────────────────
FROM nginx:alpine AS runtime
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html
EXPOSE 80
