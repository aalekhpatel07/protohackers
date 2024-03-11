class InvalidFrame(Exception):
    """Raised when we encounter an invalid frame.

    """
    kind: bytes
