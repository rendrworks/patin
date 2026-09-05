# NixOS module for Patin.
#
# Patin is a toolkit rather than a desktop environment, so this module wires up
# the pieces a system actually has to know about — the PAM policy the lock
# screen refuses to run without, the greetd session that hosts the greeter, and
# where the Lua configuration lives — and leaves everything else to the
# packages themselves.
#
# The compositor is deliberately not defaulted. 0xin lives outside nixpkgs, and
# Patin is meant to stay usable without it, so `services.patin.compositor` is a
# package you supply.
self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.patin;

  # The packaged greetd session command, with each of its overridable defaults
  # bound to a store path. data/greetd/patin-login-session.sh writes every
  # value as `: "${VAR:=default}"`, so setting them here is all it takes — the
  # script's own WLR_BACKENDS/LIBSEAT_BACKEND/`unset WAYLAND_DISPLAY` handling
  # is reused rather than reimplemented.
  greeterSession = pkgs.writeShellScript "patin-greeter-session" ''
    export PATIN_LOGIN_COMPOSITOR=${lib.escapeShellArg (lib.getExe cfg.compositor)}
    export PATIN_LOGIN_BIN=${lib.escapeShellArg "${cfg.package}/bin/patin-login"}
    export PATIN_LOGIN_SESSION=${lib.escapeShellArg (lib.getExe cfg.compositor)}
    export PATIN_LOGIN_STATE=${lib.escapeShellArg cfg.greeter.stateFile}
    ${lib.optionalString (cfg.greeter.compositorConfig != null) ''
      export OXIN_CONFIG=${lib.escapeShellArg cfg.greeter.compositorConfig}
    ''}
    exec ${cfg.package}/bin/patin-login-session
  '';

  # A wayland-sessions entry so the greeter can offer this session. Patin reads
  # these from XDG_DATA_DIRS (crates/patin-login/src/sessions.rs), which on
  # NixOS resolves to /run/current-system/sw/share.
  sessionEntry = pkgs.writeTextDir "share/wayland-sessions/patin.desktop" ''
    [Desktop Entry]
    Name=${cfg.session.name}
    Comment=Patin Wayland shell
    Exec=${cfg.session.command}
    Type=Application
  '';
in
{
  options.services.patin = {
    enable = lib.mkEnableOption "the Patin Wayland shell toolkit";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.patin;
      defaultText = lib.literalExpression "patin.packages.\${system}.patin";
      description = "The Patin package providing the shell binaries.";
    };

    compositor = lib.mkOption {
      type = lib.types.package;
      example = lib.literalExpression "pkgs.callPackage ./0xin.nix { }";
      description = ''
        The Wayland compositor that hosts Patin's surfaces, used both for the
        greeter and as the default session command.

        There is deliberately no default: 0xin is not packaged in nixpkgs, and
        Patin is designed to run under any compositor implementing the layer
        shell and session lock protocols.
      '';
    };

    config = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      example = lib.literalExpression "./init.lua";
      description = ''
        A Lua configuration file made available to every Patin program through
        `PATIN_CONFIG`.

        Patin has no system-wide configuration path of its own: it resolves
        `--config=` first, then `PATIN_CONFIG`, then
        `$XDG_CONFIG_HOME/patin/init.lua`. Setting this option is how a NixOS
        machine supplies one declaratively; a user's own
        `~/.config/patin/init.lua` is used when it is left null.
      '';
    };

    lock.enable = lib.mkEnableOption ''
      the patin-lock screen locker. This installs the PAM policy at
      /etc/pam.d/patin-lock, which patin-lock refuses to start without
    '';

    greeter = {
      enable = lib.mkEnableOption "patin-login as the greetd greeter";

      stateFile = lib.mkOption {
        type = lib.types.path;
        default = "/var/lib/greetd/patin-login-last-session";
        description = ''
          Where the greeter remembers the last username and session choice. It
          must be writable by the greetd user, which is why it does not live
          under a home directory.
        '';
      };

      compositorConfig = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = "${cfg.package}/share/patin/greetd/0xin-greeter.conf";
        defaultText = lib.literalExpression "\"\${cfg.package}/share/patin/greetd/0xin-greeter.conf\"";
        description = ''
          The compositor configuration exported as `OXIN_CONFIG` for the
          greeter session. The shipped file deliberately binds no way to launch
          a program: the greeter runs before anyone has authenticated, so a
          "spawn a terminal" keybind would be an unauthenticated shell.

          Set to null when the compositor is not 0xin.
        '';
      };
    };

    session = {
      enable = lib.mkEnableOption "a wayland-sessions entry for the Patin session";

      name = lib.mkOption {
        type = lib.types.str;
        default = "Patin";
        description = "The display name shown by greeters for this session.";
      };

      command = lib.mkOption {
        type = lib.types.str;
        default = lib.getExe cfg.compositor;
        defaultText = lib.literalExpression "lib.getExe cfg.compositor";
        description = "The command a greeter runs to start this session.";
      };
    };
  };

  config = lib.mkIf cfg.enable (lib.mkMerge [
    {
      # Patin spawns nmcli, wpctl, pactl, systemctl and loginctl by name rather
      # than by store path. They are not added to the package's closure on
      # purpose: on NixOS they land in the system profile as soon as the
      # matching service is enabled, which is the same expectation Patin has on
      # every other distribution.
      environment.systemPackages = [ cfg.package ];

      environment.sessionVariables = lib.mkIf (cfg.config != null) {
        PATIN_CONFIG = toString cfg.config;
      };
    }

    (lib.mkIf cfg.lock.enable {
      # crates/patin-lock/src/app.rs refuses to lock unless this file exists,
      # and crates/patin-lock/src/auth.rs opens the PAM service by this name.
      # The generated stack is the equivalent of data/pam/patin-lock.arch.
      security.pam.services.patin-lock = { };
    })

    (lib.mkIf cfg.greeter.enable {
      # NixOS fixes the greeter to VT1 and removed `services.greetd.vt`, so
      # there is no VT option here to mirror the `vt = 7` in
      # data/greetd/config.toml.example.
      services.greetd = {
        enable = true;
        settings.default_session.command = toString greeterSession;
      };

      # `greeter` is the user NixOS's own greetd module creates and runs the
      # session as; the state file's directory has to exist and belong to it
      # before the greeter first tries to remember a choice.
      systemd.tmpfiles.rules = [
        "d ${builtins.dirOf cfg.greeter.stateFile} 0755 greeter greeter -"
      ];
    })

    (lib.mkIf cfg.session.enable {
      environment.systemPackages = [ sessionEntry ];
    })
  ]);
}
