import logging
import sys

from ..util.paths import normalize

log = logging.getLogger(__name__)


def run_sync(settings, out=None):
    out = out or sys.stdout
    target = normalize(settings.get("target") or ".")
    action = "would sync" if settings["dry_run"] else "synced"
    log.debug("sync target %s with %d retries", target, settings["retries"])
    out.write(f"{action} {target}\n")
    return 0
