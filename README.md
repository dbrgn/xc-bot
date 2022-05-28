# XC Bot

[![CircleCI][circle-ci-badge]][circle-ci]

A chat bot that notifies you about new paragliding cross-country flights
published on [XContest](https://www.xcontest.org/) as part of the Swiss [Cross
Country Cup](https://www.xcontest.org/switzerland/de/). Inspired by the
Telegram bot [XC Switzerland](https://telegram.me/XC_Swiss_Bot) by Sergey Popov.

Supported messenger backends:

- Threema: https://threema.id/*CHXCBOT?text=help

More may follow in the future.

## Usage

Show the help screen:

    help

Follow a pilot:

    follow <username>

List pilots being followed:

    list

Stop following a pilot:

    stop <username>

Show the current bot version:

    version

## Architecture

The bot is written in Rust using a SQLite database for keeping track of the
processed flights and flight subscriptions.

## Docker Image

The repository includes a Dockerfile.

Important paths:

- `/etc/xc-bot/config.toml`: Configuration file
- `/xc-bot/`: Work directory (including database)

You'll probably want to mount both files into the container.

Note: This container runs as default user by default. If you use podman, you
can run the container as non-root.

## License

Licensed under the AGPL version 3 or later. See `LICENSE.md` file.

    Copyright (C) 2021–2022 Danilo Bargen

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as
    published by the Free Software Foundation, either version 3 of the
    License, or (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU Affero General Public License for more details.

    You should have received a copy of the GNU Affero General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

<!-- Badges -->
[circle-ci]: https://circleci.com/gh/dbrgn/xc-bot/tree/main
[circle-ci-badge]: https://circleci.com/gh/dbrgn/xc-bot/tree/main.svg?style=shield
