"""Reads `key = value` config files. Values are booleans, integers or strings."""


def _coerce(raw):
    lowered = raw.lower()
    if lowered in ("true", "false"):
        return lowered == "true"
    try:
        return int(raw)
    except ValueError:
        return raw.strip('"')


def load_config(path):
    settings = {}
    with open(path, encoding="utf-8") as handle:
        for number, line in enumerate(handle, 1):
            line = line.split("#", 1)[0].strip()
            if not line:
                continue
            if "=" not in line:
                raise ValueError(f"{path}:{number}: expected key = value")
            key, value = (part.strip() for part in line.split("=", 1))
            settings[key.replace("-", "_")] = _coerce(value)
    return settings
