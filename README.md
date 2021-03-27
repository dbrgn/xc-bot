# XC Bot

**WORK IN PROGRESS**

A chat bot that notifies you about new paragliding cross-country flights
published on [XContest](https://www.xcontest.org/) as part of the Swiss [Cross
Country Cup](https://www.xcontest.org/switzerland/de/). Inspired by the
Telegram bot [XC Switzerland](https://telegram.me/XC_Swiss_Bot) by Sergey Popov.

Supported messenger backends:

- Threema

More may follow in the future.

## Architecture

The bot is written in Rust using a SQLite database for keeping track of the
processed flights and flight subscriptions.

## License

Licensed under the AGPL version 3 or later. See `LICENSE.md` file.

    Copyright (C) 2021 Danilo Bargen

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
