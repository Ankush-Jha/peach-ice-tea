import os


def normalize(path):
    """Expand `~` and collapse `..` segments; keep relative paths relative."""
    return os.path.normpath(os.path.expanduser(path))
