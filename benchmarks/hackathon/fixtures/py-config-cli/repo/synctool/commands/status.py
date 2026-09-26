import sys


def run_status(settings, out=None):
    out = out or sys.stdout
    out.write(
        f"verbose={settings['verbose']} retries={settings['retries']} dry_run={settings['dry_run']}\n"
    )
    return 0
