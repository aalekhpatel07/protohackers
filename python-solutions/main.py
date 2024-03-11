#!/usr/bin/env python

import argparse
import asyncio
import structlog
import logging

from solutions.speed_daemon.connection_manager import ConnectionManager


logger = structlog.get_logger(__name__)


def set_up_structlog():
    structlog.configure(
        processors=[
            structlog.contextvars.merge_contextvars,
            structlog.processors.add_log_level,
            structlog.processors.StackInfoRenderer(),
            structlog.dev.set_exc_info,
            structlog.processors.TimeStamper(fmt="%Y-%m-%d %H:%M:%S", utc=False),
            structlog.dev.ConsoleRenderer()
        ],
        wrapper_class=structlog.make_filtering_bound_logger(logging.INFO),
        context_class=dict,
        logger_factory=structlog.PrintLoggerFactory(),
        cache_logger_on_first_use=False
    )
    return


SOLUTIONS = {
    "speed_daemon": lambda: ConnectionManager(),
}


def add_arguments(parser: argparse.ArgumentParser):
    parser.add_argument(
        "--port",
        type=int,
        default=9000,
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


def main():
    parser = argparse.ArgumentParser(
        description="Run TCP servers for protohackers.com"
    )
    add_arguments(parser)
    opts = parser.parse_args()
    set_up_structlog()

    # structlog.get_logger("speed_daemon", level=logging.INFO)

    logger.info(
        f"Options for {opts.problem}",
        **opts.__dict__
    )
    actual_handler = SOLUTIONS[opts.problem]().handle_connection

    async def _guarded_cb(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        addr, port = writer.get_extra_info('peername')
        if opts.allowed_ips and addr not in opts.allowed_ips:
            logger.warn(
                f"Rejecting connection attempt from {addr!r} "
                f"because it is not an allowed ip.",
                allowed_ips=opts.allowed_ips
            )
            writer.close()
            return await writer.wait_closed()
        return await actual_handler(reader, writer)

    asyncio.run(
        start_server(
            _guarded_cb,
            opts.port,
        )
    )


if __name__ == "__main__":
    main()
