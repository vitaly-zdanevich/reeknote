{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage rec {
  pname = "reeknote";
  version = (lib.importTOML ../Cargo.toml).package.version;

  src = lib.cleanSourceWith {
    src = lib.cleanSource ../.;
    filter =
      path: type:
      let
        name = baseNameOf path;
      in
      !(type == "directory" && builtins.elem name [
        ".cargo"
        ".git"
        "apt-dist"
        "deb-dist"
        "npm-dist"
        "target"
      ]);
  };

  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "--bins" ];

  meta = {
    description = "Command-line Evernote client";
    homepage = "https://gitlab.com/vitaly-zdanevich/reeknote";
    license = lib.licenses.gpl3Only;
    mainProgram = "reeknote";
    platforms = lib.platforms.linux;
  };
}
