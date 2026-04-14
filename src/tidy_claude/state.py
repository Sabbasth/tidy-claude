"""Mutable run-time state passed through the CLI."""

import click


class RunState:
    def __init__(self, debug: bool = False):
        self.debug = debug
        self.stats: dict[str, int] = {}

    def log(self, msg: str):
        if self.debug:
            click.echo(msg)

    def count(self, key: str, n: int = 1):
        self.stats[key] = self.stats.get(key, 0) + n
