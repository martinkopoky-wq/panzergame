# Dedizierter Panzer-Server als Image ohne Betriebssystem: nur die statische Binärdatei.
#
#   docker build -t panzer-server .
#   docker run -dit --name panzer -p 5555:5555/udp -e PANZER_BOTS=3 -e PANZER_ADMIN_PASS=geheim \
#       -v panzer-data:/data panzer-server
#   docker attach panzer      Konsole, zurück mit Strg+P, dann Strg+Q
#
# Einstellungen: PANZER_PORT, PANZER_BOTS, PANZER_ADMIN_PASS, PANZER_BANS, PANZER_QUIET

FROM rust:1-alpine AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p panzer-server \
    && mkdir /data

FROM scratch
COPY --from=build /src/target/release/panzer-server /panzer-server
# scratch hat kein mkdir, das Datenverzeichnis kommt deshalb aus der Build-Stufe
COPY --from=build --chown=65534:65534 /data /data
USER 65534:65534
WORKDIR /data
ENV PANZER_BANS=/data/bans.txt
VOLUME /data
EXPOSE 5555/udp
# Der Server fängt keine Signale ab, und als PID 1 würde er SIGTERM ignorieren. Bans werden
# sofort gespeichert, es gibt also nichts, was beim Beenden verloren gehen könnte.
STOPSIGNAL SIGKILL
ENTRYPOINT ["/panzer-server"]
