{ pkgs ? import <nixpkgs> { } }:

pkgs.callPackage ./nix/pkgs/emacs-i3.nix { }
