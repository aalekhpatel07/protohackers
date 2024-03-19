#!/usr/bin/env python

import argparse
import asyncio
import structlog
import logging
from solutions.speed_daemon.app import App as SpeedDaemon
from structlog.typing import EventDict, WrappedLogger


logger = structlog.get_logger(__name__)


class LogJump:

    def __call__(
        self, _logger: WrappedLogger, name: str, event_dict: EventDict
    ) -> EventDict:
        file_part = event_dict.pop("filename")
        line_no = event_dict.pop("lineno")
        event_dict["location"] = f'"{file_part}:{line_no}"'

        return event_dict


def set_up_structlog():
    structlog.configure(
        processors=[
            structlog.contextvars.merge_contextvars,
            structlog.stdlib.add_log_level,
            structlog.processors.StackInfoRenderer(),
            structlog.dev.set_exc_info,
            structlog.processors.TimeStamper(fmt="%Y-%m-%d %H:%M.%S", utc=False),
            structlog.processors.CallsiteParameterAdder(
                [
                    # add either pathname or filename and then set full_path to True or False in LogJump below
                    # structlog.processors.CallsiteParameter.PATHNAME,
                    structlog.processors.CallsiteParameter.FILENAME,
                    structlog.processors.CallsiteParameter.LINENO,
                    structlog.processors.CallsiteParameter.FUNC_NAME,
                    structlog.processors.CallsiteParameter.MODULE,
                ],
            ),
            LogJump(),
            structlog.dev.ConsoleRenderer(),
        ],
        wrapper_class=structlog.make_filtering_bound_logger(logging.NOTSET),
        context_class=dict,
        logger_factory=structlog.PrintLoggerFactory(),
        cache_logger_on_first_use=False
    )
    return


SOLUTIONS = {
    "speed_daemon": lambda: SpeedDaemon(),
}


def add_arguments(parser: argparse.ArgumentParser):
    parser.add_argument(
        "--port",
        type=int,
        default=8300,
        help="The port to bind for this server."
    )
    parser.add_argument(
        "--allowed-ips",
        type=str,
        nargs="*",
        help="If provided, only connection attempts from these ips will be honoured.",
        default=[
            "127.0.0.1",  # for testing against local clients.
            "206.189.113.124"  # The real protohackers client: https://protohackers.com/security
        ]
    )
    parser.add_argument(
        "problem",
        choices=SOLUTIONS.keys(),
        type=str,
        help="The solution to start a server for.",
        default="speed_daemon",
    )
    return


async def start_server(cb, port):
    server = await asyncio.start_server(cb, host="0.0.0.0", port=port)
    addrs = ', '.join(str(sock.getsockname()) for sock in server.sockets)
    logger.info(f'Serving on {addrs}')
    async with server:
        await server.serve_forever()


async def main():
    parser = argparse.ArgumentParser(
        description="Run TCP servers for protohackers.com"
    )
    add_arguments(parser)
    opts = parser.parse_args()
    set_up_structlog()

    # # structlog.get_logger("common", level=logging.CRITICAL)
    # # structlog.get_logger("common.server", level=logging.DEBUG)
    # structlog.get_logger("speed_daemon", level=logging.DEBUG)

    logger.info(
        f"Options for {opts.problem}",
        **opts.__dict__
    )
    app = SOLUTIONS[opts.problem]()
    async with asyncio.TaskGroup() as tg:
        app_run = tg.create_task(app.run())
        handle_connection = tg.create_task(start_server(app.server.handle_connection, opts.port))

    logger.info(
        f"TaskGroup completed:\n{app_run}\n{handle_connection}"
    )

if __name__ == "__main__":
    asyncio.run(main())

