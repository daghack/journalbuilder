# syntax=docker/dockerfile:1.7
#
# The journal editor (webapp/) as a static site, served by nginx. It sits
# at the repository root, its build context, since the renderer is the
# whole crate:
#
#   docker build -t journal-editor .
#   docker run --rm -p 8080:80 journal-editor
#
# then open http://localhost:8080. Darkly previews and exports use WebGPU,
# which browsers only offer on localhost or over HTTPS: anywhere else, put
# the container behind TLS. The flow simulator (tools/flowsim) is served
# beside it, at http://localhost:8080/flow/.

# ---- The renderer, scribe-wasm, and the flow simulator built to WebAssembly ----
FROM --platform=$BUILDPLATFORM rust:1.99-trixie AS wasm
RUN rustup target add wasm32-unknown-unknown
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install wasm-pack --version 0.13.1 --locked
WORKDIR /src
# Only what the crate builds from, so other changes don't rebuild it.
COPY Cargo.toml Cargo.lock build.rs ./
COPY src src
COPY webapp/library webapp/library
COPY webapp/wasm webapp/wasm
# A workspace member, so cargo needs it to build either.
COPY tools/flowsim/sim tools/flowsim/sim
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    --mount=type=cache,target=/root/.cache/.wasm-pack \
    wasm-pack build webapp/wasm --target web --release --out-dir /out/wasm-pkg --out-name scribe_wasm
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    --mount=type=cache,target=/root/.cache/.wasm-pack \
    wasm-pack build tools/flowsim/sim --target web --release --out-dir /out/flowsim-pkg --out-name flowsim --features bindings

# ---- The site: the Angular app, with the renderer and the examples ----
FROM --platform=$BUILDPLATFORM node:24.21-trixie-slim AS site
ENV NG_CLI_ANALYTICS=false
WORKDIR /src/webapp/frontend
COPY webapp/frontend/package.json webapp/frontend/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY webapp/frontend ./
# The journals built into the editor; it opens the rest from a folder.
COPY webapp/journals/showcase /src/webapp/journals/showcase
COPY webapp/journals/api_reference /src/webapp/journals/api_reference
# examples.mjs also copies the library's files, for the unit tests' fixtures.
COPY webapp/library /src/webapp/library
COPY --from=wasm /out/wasm-pkg wasm-pkg
# The Fluid tab embeds the flow simulator's view (@flowsim/* in tsconfig.json),
# its engine served beside the renderer's.
COPY tools/flowsim/web/src /src/tools/flowsim/web/src
COPY --from=wasm /out/flowsim-pkg /src/tools/flowsim/web/wasm-pkg
COPY --from=wasm /out/flowsim-pkg/flowsim_bg.wasm wasm-pkg/
# Gzipped copies sit beside the files for nginx's gzip_static: the
# renderer is 17 MB, 9 MB gzipped, and isn't worth compressing per request.
RUN npm run examples && npx ng build \
    && find dist/journal-editor/browser -type f \
        \( -name '*.wasm' -o -name '*.js' -o -name '*.css' -o -name '*.html' -o -name '*.json' -o -name '*.rhai' -o -name '*.ico' \) \
        -exec gzip -9 -k {} +

# ---- The flow simulator's page ----
FROM --platform=$BUILDPLATFORM node:24.21-trixie-slim AS flow
WORKDIR /src/tools/flowsim/web
COPY tools/flowsim/web/package.json tools/flowsim/web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY tools/flowsim/web ./
COPY --from=wasm /out/flowsim-pkg wasm-pkg
RUN npx tsc --noEmit && npx vite build \
    && find dist -type f \( -name '*.wasm' -o -name '*.js' -o -name '*.css' -o -name '*.html' \) \
        -exec gzip -9 -k {} +

# ---- Serving them ----
FROM nginx:1.30-alpine
COPY webapp/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=site /src/webapp/frontend/dist/journal-editor/browser /usr/share/nginx/html
COPY --from=flow /src/tools/flowsim/web/dist /usr/share/nginx/html/flow
EXPOSE 80
