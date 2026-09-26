# Settings from the config file are ignored

`synctool` reads defaults from a TOML-like config file (`--config PATH`) and lets command-line
flags override them. Several users report that settings in the config file have no effect unless
they repeat them on the command line. For example, with this config file:

```
verbose = true
retries = 5
```

running `synctool --config sync.conf status` prints `verbose=False retries=3`; it should print
`verbose=True retries=5`. Passing `--retries 7` on the command line must still win over the file,
and with no config file at all the built-in defaults (`verbose=False`, `retries=3`,
`dry_run=False`) still apply.

The precedence is: built-in defaults < config file < command-line flags that were actually given.
The tests in `tests/` describe the expected behaviour.
