"""Command-line entry point."""

import argparse

from . import __version__
from .commands import COMMANDS
from .config.loader import load_config
from .config.merge import merge_settings
from .util.logging import configure_logging


def build_parser():
    parser = argparse.ArgumentParser(prog="synctool")
    parser.add_argument("--version", action="version", version=f"synctool {__version__}")
    parser.add_argument("--config", help="path to a config file")
    parser.add_argument("--verbose", action="store_true", help="log every file considered")
    parser.add_argument("--retries", type=int, default=3, help="network retries per file")
    parser.add_argument("--dry-run", action="store_true", help="report changes without making them")
    parser.add_argument("command", choices=sorted(COMMANDS))
    return parser


def main(argv=None, out=None):
    args = build_parser().parse_args(argv)
    file_settings = load_config(args.config) if args.config else {}
    settings = merge_settings(file_settings, vars(args))
    configure_logging(settings["verbose"])
    return COMMANDS[args.command](settings, out)
