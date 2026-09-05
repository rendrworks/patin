{
  description = "Patin — a native Rust toolkit for building Wayland graphical shells";

  inputs = {
    # `flake.lock` is what pins the revision; naming the branch here is what
    # `nix flake update` follows.
    #
    # The sibling `termworks/oslo` flake hardcodes a revision in this URL
    # instead, and its April 2026 pin cannot build a Rust package any more:
    # that nixpkgs fetches crates from `crates.io/api/v1`, which Cloudflare now
    # answers with 403 for any User-Agent containing `curl/` — exactly what
    # nixpkgs' `fetchurl` sends. Later revisions fetch from `static.crates.io`
    # instead. See https://github.com/rust-lang/crates.io/issues/13482.
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      manifest = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      version = manifest.package.version;

      # Every crate in the workspace that produces a binary. The bin name and
      # the crate name are the same for all seven, so `-p <crate>` selects a
      # binary unambiguously.
      binaryCrates = [
        "patin-launcher"
        "patin-lock"
        "patin-login"
        "patin-network-settings"
        "patin-osk"
        "patin-session"
        "patin-workspaces-bar"
      ];
    in
    flake-utils.lib.eachSystem systems (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        lib = pkgs.lib;

        # **The toolchain, read from the file rustup and CI already obey.**
        #
        # `rust-toolchain.toml` stays the one place the version lives: rustup
        # honours it on a normal distribution, `.github/workflows/ci.yml` picks
        # it up through `rustup show`, and this line means Nix agrees with both
        # instead of introducing a second number that nothing checks.
        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # Build the packages with that same toolchain rather than whichever
        # rustc nixpkgs happens to ship, so `nix build` and `cargo build` are
        # compiling with the identical compiler.
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };

        # The whole native dependency set, and nothing more. Everything else
        # Patin touches is pure Rust: the Wayland protocol is spoken by
        # `wayland-backend`'s Rust implementation rather than libwayland, fonts
        # go through `fontconfig-parser` rather than libfontconfig, D-Bus
        # through `zbus` rather than libdbus, and Lua through `luna`, a Rust
        # VM. That leaves exactly what CI apt-installs: xkbcommon and PAM.
        nativeLibraries = [
          pkgs.libxkbcommon
          pkgs.pam
        ];

        mkPatin = lib.makeOverridable (
          {
            pname ? "patin",
            # Which binary crates to build. Narrow this to build only what a
            # particular machine needs — a greeter host wants `patin-login`
            # and nothing else.
            crates ? binaryCrates,
            # Examples are opt-in and never built by default; `demo_bar` is a
            # demonstration of the toolkit, not a shipped program.
            examples ? [ ],
          }:
          rustPlatform.buildRustPackage {
            inherit pname version;
            src = ./.;

            # `luna` is the workspace's only git dependency (see
            # crates/patin-lua/Cargo.toml), and a git dependency cannot be
            # fetched from a plain `cargoHash` — it needs its own output hash.
            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                "luna-0.5.1" = "sha256-gCdX/Ni2RbK69KMJFVzK72QSuDFVVQM2deiKXH3AvoI=";
              };
            };

            # smithay-client-toolkit's `xkbcommon` feature runs
            # `pkg_config::Config::new().find("xkbcommon")` in its build script.
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = nativeLibraries;

            # `--example` is a target selector: on its own it tells cargo to
            # build *only* that example, so `--bins` has to come with it or the
            # binary crates listed alongside would silently not be built.
            cargoBuildFlags =
              lib.concatMap (crate: [ "-p" crate ]) crates
              ++ lib.optionals (examples != [ ]) (
                [ "-p" "patin" ]
                ++ lib.concatMap (example: [ "--example" example ]) examples
                ++ lib.optional (crates != [ ]) "--bins"
              );

            # Test exactly what was built. CI is what runs the full workspace
            # suite; the suite is headless, so it is safe in the sandbox.
            cargoTestFlags = lib.concatMap (crate: [ "-p" crate ]) crates;
            doCheck = crates != [ ];

            postInstall = ''
              ${lib.concatMapStringsSep "\n" (example: ''
                built=$(find target -type f -perm -111 -path '*/release/examples/${example}' -print -quit)
                if [ -z "$built" ]; then
                  echo "error: example ${example} was not built" >&2
                  exit 1
                fi
                # scripts/install-demo-user.sh installs the demo bar under this
                # name, so the Nix package agrees with it.
                install -Dm0755 "$built" "$out/bin/patin"
              '') examples}

              ${lib.optionalString (lib.elem "patin-login" crates) ''
                # The greetd session command, with its two filesystem defaults
                # repointed into the store. Every value in the script is a
                # `: "''${VAR:=default}"`, so the NixOS module can still
                # override each one without the script being patched further.
                # Only shipped when the greeter it launches was built.
                install -Dm0644 data/greetd/0xin-greeter.conf -t "$out/share/patin/greetd"
                install -Dm0644 data/greetd/config.toml.example -t "$out/share/patin/greetd"
                install -Dm0755 data/greetd/patin-login-session.sh "$out/bin/patin-login-session"
                substituteInPlace "$out/bin/patin-login-session" \
                  --replace-fail /usr/local/bin/patin-login "$out/bin/patin-login" \
                  --replace-fail /etc/greetd/0xin-greeter.conf "$out/share/patin/greetd/0xin-greeter.conf"
              ''}

              ${lib.optionalString (lib.elem "patin-lock" crates) ''
                # The PAM policies to adapt. Never installed to /etc by the
                # package: PAM policy is system security configuration.
                install -Dm0644 data/pam/patin-lock.* -t "$out/share/patin/pam"
              ''}

              install -Dm0644 data/patin/init.lua.example -t "$out/share/patin"
            '';

            # Oslo asserts its binaries have *no* dynamic dependencies. Patin
            # asserts the opposite: PAM dlopens its modules, so libpam must stay
            # dynamic or `patin-lock` could never authenticate anyone. This
            # catches a silently emptied `buildInputs`.
            doInstallCheck = true;
            nativeInstallCheckInputs = [ pkgs.binutils ];
            installCheckPhase = ''
              runHook preInstallCheck

              for binary in ${lib.escapeShellArgs crates}; do
                test -x "$out/bin/$binary" || { echo "error: missing $binary" >&2; exit 1; }
              done

              ${lib.optionalString (lib.elem "patin-lock" crates) ''
                needed=$(readelf -d "$out/bin/patin-lock")
                for library in libpam libxkbcommon; do
                  case "$needed" in
                    *"$library"*) ;;
                    *) echo "error: patin-lock is not linked against $library" >&2; exit 1 ;;
                  esac
                done
              ''}

              runHook postInstallCheck
            '';

            meta = {
              description = "A native Rust toolkit for building Wayland graphical shells";
              homepage = "https://github.com/termworks/patin";
              license = lib.licenses.mit;
              platforms = lib.platforms.linux;
            }
            // lib.optionalAttrs (examples != [ ]) { mainProgram = "patin"; }
            // lib.optionalAttrs (examples == [ ] && lib.length crates == 1) {
              mainProgram = lib.head crates;
            };
          }
        );

        patin = mkPatin { };

        # The toolkit demonstration from `examples/`, installed as `patin` the
        # way scripts/install-demo-user.sh installs it. It spawns
        # patin-network-settings (examples/demo_bar/scene.rs), so that binary
        # comes along.
        patin-demo-bar = mkPatin {
          pname = "patin-demo-bar";
          crates = [ "patin-network-settings" ];
          examples = [ "demo_bar" ];
        };

        mkApp = package: binary: {
          type = "app";
          program = "${package}/bin/${binary}";
          meta.description = "Run ${binary}";
        };
      in
      {
        packages = {
          inherit patin patin-demo-bar;
          default = patin;
        };

        # No `default`: none of the seven binaries is "the" program, so naming
        # one is part of the command.
        apps = lib.listToAttrs (
          map (crate: lib.nameValuePair crate (mkApp patin crate)) binaryCrates
        )
        // {
          patin-demo-bar = mkApp patin-demo-bar "patin";
        };

        checks = {
          inherit patin patin-demo-bar;

          fmt = pkgs.runCommand "patin-fmt" { nativeBuildInputs = [ toolchain ]; } ''
            cp -r ${./.} source
            chmod -R u+w source
            cd source
            export CARGO_HOME=$TMPDIR/cargo
            cargo fmt --all -- --check
            touch $out
          '';
        };

        devShells.default = pkgs.mkShell {
          packages = [
            toolchain
            pkgs.rust-analyzer
            pkgs.pkg-config
            pkgs.mdbook
            pkgs.git
          ];
          buildInputs = nativeLibraries;
        };
      }
    )
    // {
      nixosModules.default = import ./nix/module.nix self;
      nixosModules.patin = self.nixosModules.default;

      overlays.default = final: _prev: {
        patin = self.packages.${final.system}.patin;
        patin-demo-bar = self.packages.${final.system}.patin-demo-bar;
      };
    };
}
