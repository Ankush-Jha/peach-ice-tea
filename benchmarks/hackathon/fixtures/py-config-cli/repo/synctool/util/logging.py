import logging


def configure_logging(verbose):
    logging.basicConfig(level=logging.DEBUG if verbose else logging.WARNING, force=True)
