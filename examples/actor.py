#!/usr/bin/env python3

import sys
import argparse

def on_free(args, actor_name):
    """
    Function called when the state of the connected machine changes to Free
    again
    """
    if args.verbose > 2:
        print("on_free called!")

    if actor_name == "DoorControl1":
        # Do whatever you want to do in case `DoorControl1` is returned back to free.
        # Keep in mind that process actors should return quickly to not miss
        # updates, so if you need to do things that take a while fork a new
        # process e.g. with the `subprocess` Module
        print("I'm locking door 1!")
        pass
    elif actor_name == "DoorControl2":
        print("I'm locking door 2!")
        pass # Close a different door
    else:
        if not args.quiet:
            print("process called with unknown id %s for state `Free`" % actor_name)
        # It's a good idea to exit with an error code in case something
        # unexpected happens.
        # The process module logs everything printed to stdout by actors into
        # the server log, but marks them as `Error` in case the actor process
        # exits with a code != 0, making debugging somewhat easier.
        exit(-1)

def on_use(args, actor_name, user_id):
    """
    Function called when an user takes control of the connected machine

    user_id contains the UID of the user now using the machine
    """
    if args.verbose > 2:
        print("on_use called!")
    if actor_name == "DoorControl1":
        print("I'm opening door 1 for 10 seconds!")
        pass # Open door one
    elif actor_name == "DoorControl2":
        print("I'm opening door 2 for 10 seconds!")
        pass # Open a different door
    else:
        if not args.quiet:
            print("process called with unknown id %s for state `InUse`" % actor_name)
        # It's a good idea to exit with an error code in case something
        # unexpected happens.
        # The process module logs everything printed to stdout by actors into
        # the server log, but marks them as `Error` in case the actor process
        # exits with a code != 0, making debugging somewhat easier.
        exit(-1)

def on_tocheck(args, actor_name, user_id):
    """
    Function called when an user returns control and the connected machine is
    configured to go to state `ToCheck` instead of `Free` in that case.

    user_id contains the UID of the manager expected to check the machine.
    The user that used the machine beforehand has to be taken from the last
    user field using the API (via e.g. the mobile app)
    """
    if args.verbose > 2:
        print("on_tocheck called!")
    if not args.quiet:
        print("process called with unexpected combo id %s and state 'ToCheck'" % actor_name)
    exit(-1)

def on_blocked(args, actor_name, user_id):
    """
    Function called when an manager marks the connected machine as `Blocked`

    user_id contains the UID of the manager that blocked the machine
    """
    if args.verbose > 2:
        print("on_blocked called!")
    if not args.quiet:
        print("process called with unexpected combo id %s and state 'Blocked'" % actor_name)
    exit(-1)

def on_disabled(args, actor_name):
    """
    Function called when the connected machine is marked `Disabled`
    """
    if not args.quiet:
        print("process called with unexpected combo id %s and state 'Disabled'" % actor_name)
    exit(-1)

def on_reserve(args, actor_name, user_id):
    """
    Function called when the connected machine has been reserved by somebody.

    user_id contains the UID of the reserving user.
    """
    if not args.quiet:
        print("process called with unexpected combo id %s and state 'Reserved'" % actor_name)
    exit(-1)


def main(args):
    """
    Python example actor

    This is an example how to use the `process` actor type to run a Python script.
    """

    if args.verbose is not None:
        if args.verbose == 1:
            print("verbose output enabled")
        elif args.verbose == 2:
            print("loud output enabled!")
        elif args.verbose == 3:
            print("LOUD output enabled!!!")
        elif args.verbose > 4:
            print("Okay stop you're being ridiculous.")
            sys.exit(-2)
    else:
        args.verbose = 0

    # You could also check the actor name here and call different functions
    # depending on that variable instead of passing it to the state change
    # methods.

    new_state = args.state
    if new_state == "free":
        on_free(args, args.name)
    elif new_state == "inuse":
        on_use(args, args.name, args.userid)
    elif new_state == "tocheck":
        on_tocheck(args, args.name, args.userid)
    elif new_state == "blocked":
        on_blocked(args, args.name, args.userid)
    elif new_state == "disabled":
        on_disabled(args, args.name)
    elif new_state == "reserved":
        on_reserve(args, args.name, args.userid)
    else:
        print("Process actor called with unknown state %s" % new_state)

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    # Parameters are passed to the Process actor as follows:
    # 1. the contents of params.args, split by whitespace as separate args
    # 2. the configured id of the actor (e.g. "DoorControl1")
    # 3. the new state as one of [free|inuse|tocheck|blocked|disabled|reserved]

    parser.add_argument("-q", "--quiet", help="be less verbose", action="store_true")
    parser.add_argument("-v", "--verbose", help="be more verbose", action="count")

    parser.add_argument("name",
                        help="name of this actor as configured in bffh.dhall"
                        )

    # We parse the new state using subparsers so that we only require a userid
    # in case it's a state that sets one.
    subparsers = parser.add_subparsers(required=True, dest="state")

    parser_free = subparsers.add_parser("free")

    parser_inuse = subparsers.add_parser("inuse")
    parser_inuse.add_argument("userid", help="The user that is now using the machine")

    parser_tocheck = subparsers.add_parser("tocheck")
    parser_tocheck.add_argument("userid", help="The user that should go check the machine")

    parser_blocked = subparsers.add_parser("blocked")
    parser_blocked.add_argument("userid", help="The user that marked the machine as blocked")

    parser_disabled = subparsers.add_parser("disabled")

    parser_reserved = subparsers.add_parser("reserved")
    parser_reserved.add_argument("userid", help="The user that reserved the machine")

    args = parser.parse_args()
    main(args)
