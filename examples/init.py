#!/usr/bin/env python

import sys
import time

while True:
    print('{ "state": { "1.3.6.1.4.1.48398.612.2.4": { "state": "Free" } } }')
    sys.stdout.flush()
    time.sleep(2)

    print('{ "state": { "1.3.6.1.4.1.48398.612.2.4": { "state": { "InUse": { "id": "Testuser" } } } } }')
    sys.stdout.flush()
    time.sleep(2)
