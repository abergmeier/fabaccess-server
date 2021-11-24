#!/bin/bash

CONTAINER_ALREADY_STARTED="/var/lib/bffh/firststartflag"
if [ ! -e $CONTAINER_ALREADY_STARTED ]; then
    touch $CONTAINER_ALREADY_STARTED
    echo "-- Seeding Database --"
    diflouroborane -c /etc/bffh/bffh.dhall --load=/etc/bffh
else
    diflouroborane -c /etc/bffh/bffh.dhall
fi
