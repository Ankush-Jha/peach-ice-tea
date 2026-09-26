"""Combines built-in defaults, config-file settings and command-line arguments."""

from .defaults import DEFAULTS

# argparse keys that are not settings.
_NOT_SETTINGS = {"command", "config"}


def merge_settings(file_settings, cli_args):
    """Return the effective settings: defaults, then the file, then the command line."""
    settings = dict(DEFAULTS)
    settings.update(file_settings)
    for key, value in cli_args.items():
        if key in _NOT_SETTINGS:
            continue
        settings[key] = value
    return settings
