# Build configuration
ARG project_name=fake_haproxy

# Set up rust build environment
FROM clux/muslrust:stable AS build
ARG project_name

# Create layer for the dependencies, so we don't have to rebuild them later
WORKDIR /usr/src
RUN USER=root cargo new $project_name
WORKDIR /usr/src/$project_name
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

# Build the actual source
COPY src ./src
RUN touch ./src/main.rs && cargo build --release

# Create a minimal docker file with only the resulting binary
FROM scratch
ARG project_name
COPY --from=build /usr/src/$project_name/target/x86_64-unknown-linux-musl/release/$project_name ./app
USER 1000
CMD ["./app"]
