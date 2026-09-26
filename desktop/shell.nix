# The system libraries GPUI needs on Linux, for NixOS. mise supplies the
# Rust toolchain; this supplies what it cannot:
#
#   mise run desktop        (runs `nix-shell desktop/shell.nix --run ...`)
#
{ pkgs ? import <nixpkgs> { } }:

let
  libs = with pkgs; [
    fontconfig
    freetype
    libxkbcommon
    vulkan-loader
    wayland
    libx11
    libxcb
    libxcursor
    libxi
    libxrandr
  ];
in
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [ pkg-config ];
  buildInputs = libs;
  # GPUI loads Vulkan, Wayland, and X11 at run time with dlopen.
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libs;
}
