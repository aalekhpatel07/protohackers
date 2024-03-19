#!/usr/bin/sh

PG_HOST=localhost
PG_USER=root

generate_model() {
  # Make sure this python
  # has peewee installed.
  echo "Inspecting and generating models for schema: $1"
  python -m pwiz \
    --engine=postgresql \
    --user=${PG_USER} \
    --host=${PG_HOST} \
    --schema="$1" \
    --info \
    --preserve-order \
    --password \
    protohackers \
    > "$1"/state_machine/models.py

  echo "Linting $1/state_machine/models.py"
  python -m black \
    "$1"/state_machine/models.py
}

SCHEMA=speed_daemon
for schema in $SCHEMA; do generate_model $schema; done
