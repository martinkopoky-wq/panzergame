# Das Docker-Image ohne Betriebssystem, Gegenstück zum Dockerfile:
#
#   nix build .#panzer-server-image && docker load < result
#
# Im Image liegt nur, was der Server wirklich referenziert, kein /bin und keine Shell.
{ lib, dockerTools, panzer-server }:

dockerTools.buildLayeredImage {
  name = "panzer-server";
  tag = panzer-server.version;

  extraCommands = "mkdir -p data";
  fakeRootCommands = "chown 65534:65534 data";

  config = {
    Entrypoint = [ (lib.getExe panzer-server) ];
    User = "65534:65534";
    WorkingDir = "/data";
    Env = [ "PANZER_BANS=/data/bans.txt" ];
    Volumes."/data" = { };
    ExposedPorts."5555/udp" = { };
    # Der Server fängt keine Signale ab, und als PID 1 würde er SIGTERM ignorieren. Bans werden
    # sofort gespeichert, es gibt also nichts, was beim Beenden verloren gehen könnte.
    StopSignal = "SIGKILL";
  };
}
