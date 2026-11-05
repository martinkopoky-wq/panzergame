# Bibliotheken, die miniquad und winit/glutin erst zur Laufzeit per dlopen laden. Der Linker
# sieht sie nicht, deshalb kommen sie von Hand in den RUNPATH der Programme.
pkgs: with pkgs; [
  libGL
  libx11
  libxcursor
  libxi
  libxkbcommon
  libxrandr
  wayland
]
