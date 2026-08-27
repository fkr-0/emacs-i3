{ lib, rustPlatform }:

let
  cargoToml = builtins.fromTOML (builtins.readFile ../../Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = "emacs-i3";
  version = cargoToml.package.version;

  src = lib.cleanSource ../..;

  cargoLock.lockFile = ../../Cargo.lock;

  postInstall = ''
    install -Dm644 ${../../elisp/emacs-i3.el} \
      "$out/share/emacs/site-lisp/emacs-i3.el"
  '';

  meta = with lib; {
    description = "Emacs i3 unified window management";
    homepage = "https://github.com/fkr-0/emacs-i3";
    license = licenses.mit;
    mainProgram = "emacs-i3";
  };
}
