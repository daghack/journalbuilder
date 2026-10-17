# syntax=docker/dockerfile:1.7
#
# The journal editor (webapp/) as a static site, served by nginx. Build it
# from the repository root, since the renderer is the whole crate:
#
#   docker build -f webapp/Dockerfile -t journal-editor .
#   docker run --rm -p 8080:80 journal-editor
#
# then open http://localhost:8080. Darkly previews and exports use WebGPU,
# which browsers only offer on localhost or over HTTPS: anywhere else, put
# the container behind TLS.

# ---- The renderer: scribe-wasm built to WebAssembly ----
FROM --platform=$BUILDPLATFORM rust:1.99-trixie AS wasm
RUN rustup target add wasm32-unknown-unknown
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install wasm-pack --version 0.13.1 --locked
WORKDIR /src
# Only what the crate builds from, so other changes don't rebuild it.
COPY Cargo.toml Cargo.lock build.rs ./
COPY src src
COPY config config
COPY layouts layouts
COPY locks locks
COPY webapp/wasm webapp/wasm
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    --mount=type=cache,target=/root/.cache/.wasm-pack \
    wasm-pack build webapp/wasm --target web --release --out-dir /out/wasm-pkg --out-name scribe_wasm

# ---- The site: the Angular app, with the renderer and the examples ----
FROM --platform=$BUILDPLATFORM node:24.21-trixie-slim AS site
ENV NG_CLI_ANALYTICS=false
WORKDIR /src/webapp/frontend
COPY webapp/frontend/package.json webapp/frontend/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY webapp/frontend ./
COPY journals /src/journals
# examples.mjs also copies these, for the unit tests' fixtures.
COPY config /src/config
COPY locks /src/locks
COPY --from=wasm /out/wasm-pkg wasm-pkg
# Gzipped copies sit beside the files for nginx's gzip_static: the
# renderer is 17 MB, 9 MB gzipped, and isn't worth compressing per request.
RUN npm run examples && npx ng build \
    && find dist/journal-editor/browser -type f \
        \( -name '*.wasm' -o -name '*.js' -o -name '*.css' -o -name '*.html' -o -name '*.json' -o -name '*.rhai' -o -name '*.ico' \) \
        -exec gzip -9 -k {} +

# ---- Serving it ----
FROM nginx:1.30-alpine
COPY webapp/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=site /src/webapp/frontend/dist/journal-editor/browser /usr/share/nginx/html
EXPOSE 80
