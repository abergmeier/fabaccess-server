{- Main configuration file for bffh
 - ================================
 -
 - In this configuration file you configure almost all parts of how bffh operates, but most importantly:
 -      * Machines
 -      * Initiators and Actors
 -      * Which Initiators and Actors relate to which machine(s)
 -      * Roles and the permissions granted by them
 -}

-- The config is in the configuration format/language dhall. You can find more information about dhall over at
-- https://dhall-lang.org

-- (Our) Dhall is somewhat similar to JSON and YAML in that it expects a top-level object containing the
-- configuration values
{
    -- Configure the addresses and ports bffh listens on
    listens = [
        -- BFFH binds a port for every listen object in this array.
        -- Each listen object is of the format { address = <STRING>, port = <INTEGER> }
        -- If you don't specify a port bffh will use the default of `59661`
        -- 'address' can be a IP address or a hostname
        -- If bffh can not bind a port for the specified combination if will log an error but *continue with the remaining ports*
        { address = "127.0.0.1", port = 59661 },
        { address = "::1", port = 59661 },
        { address = "192.168.0.114", port = 59661 }
    ],

    -- Configure TLS. BFFH requires a PEM-encoded certificate and the associated key as two separate files
    certfile = "examples/self-signed-cert.pem",
    keyfile = "examples/self-signed-key.pem",

    -- BFFH right now requires a running MQTT broker.
    mqtt_url = "tcp://localhost:1883",

    -- Path to the database file for bffh. bffh will in fact create two files; ${db_path} and ${db_path}.lock.
    -- BFFH will *not* create any directories so ensure that the directory exists and the user running bffh has write
    -- access into them.
    db_path = "/tmp/bffh",

    -- Audit log path. Bffh will log state changes into this file, one per line.
    -- Audit log entries are for now JSON:
    -- {"timestamp":1641497361,"machine":"Testmachine","state":{"state":{"InUse":{"uid":"Testuser","subuid":null,"realm":null}}}}
    auditlog_path = "/tmp/bffh.audit",

    -- In dhall you can also easily import definitions from other files, e.g. you could write
    -- roles = ./roles.dhall
    roles = {
        -- Role definitions
        -- A role definition is of the form
        -- rolename = {
        --    parents = [<list of role names to inherit from>],
        --    permissions = [<list of perm rules>],
        -- }
        --
        -- Role names are case sensitive, so RoleName != rolename.
        --
        -- If you want either parents or permissions to be empty its best to completely skip it:
        testrole = {
            permissions = [ "lab.some.admin" ]
        },
        somerole = {
            parents = ["testparent"],
            -- "Permissions" are formatted as Perm Rules, so you can use the wildcards '*' and '+'
            permissions = [ "lab.test.*" ]
        },
        -- Roles can inherit from each other. In that case a member of e.g. 'somerole' that inherits from
        -- 'testparent' will have all the permissions of 'somerole' AND 'testparent' assigned to them.
        -- Right now permissions are stricly additive so you can't take a permission away in a child role that a parent
        -- role grants.
        testparent = {
            permissions = [
                "lab.some.write",
                "lab.some.read",
                "lab.some.disclose"
            ]
        }
    },

    -- Configure machines
    -- "Machines" (which in future will be more appropiately named "resources") are the main thing bffh is concerned
    -- with.
    -- You can define an almost limitless amount of machines (well 2^64 - 1, so 18_446_744_073_709_551_615 to be precise)
    -- Each of these machines can then have several "actors" and "initiators" assigned
    machines = {
        Testmachine = {
            -- A machine comes with two "names". The id above ("Testmachine") and the "name" ("MachineA").
            -- The id is what you'll use in the config format and is strictly limited to alphanumeric characters and '_'
            -- and must begin with a letter. Most importantly you CAN NOT use '-' or spaces in an identifier
            -- (dhall makes this technically possible but you can break things in subtle ways)

            -- REQUIRED. The "name" of a machine is what will be presented to humans. It can contain all unicode
            -- including spaces and nonprintable characters.
            -- A name SHOULD be short but unique.
            name = "MachineA",

            -- OPTIONAL. A description can be assigned to machines. It will also only be shown to humans. Thus it is
            -- once again limited only to unicode. If you want to provide your users with important additional
            -- information other than the name this is the place to do it.
            description = "A test machine",

            -- OPTIONAL. If you have a wiki going into more detail how to use a certain machine or what to keep in
            -- mind when using it you can provide a URL here that will be presented to users.
            wiki = "https://wiki.example.org/machineA",

            -- OPTIONAL. You can assign categories to machines to allow clients to group/filter machines by them.
            category = "Testcategory",

            -- REQUIRED.
            -- Each machine MUST have *all* Permission levels assigned to it.
            -- Permissions aren't PermRules as used in the 'roles' definitions but must be precise without wildcards.
            -- Permission levels aren't additive, so a user having 'manage' permission does not automatically get
            -- 'read' or 'write' permission.

            -- (Note, disclose is not fully implemented at the moment)
            -- Users lacking 'disclose' will not be informed about this machine in any way and it will be hidden from
            -- them in the client. Usually the best idea is to assign 'read' and 'disclose' to the same permission.
            disclose = "lab.test.read",

            -- Users lacking 'read' will be shown a machine including name, description, category and wiki but not
            -- it's current state. The current user is not disclosed.
            read = "lab.test.read",

            -- The 'write' permission allows to 'use' the machine.
            write = "lab.test.write",

            -- Manage represents the 'superuser' permission. Users with this permission can force set any state and
            -- read out the current user
            manage = "lab.test.admin"
        },
        Another = {
            wiki = "test_another",
            category = "test",
            disclose = "lab.test.read",
            manage = "lab.test.admin",
            name = "Another",
            read = "lab.test.read",
            write = "lab.test.write"
        },
        Yetmore = {
            description = "Yet more test machines",
            disclose = "lab.test.read",
            manage = "lab.test.admin",
            name = "Yetmore",
            read = "lab.test.read",
            write = "lab.test.write"
        }
    },

    -- Actor configuration. Actors are how bffh affects change in the real world by e.g. switching a power socket
    -- using a shelly
    actors = {
        -- Actors similarly to machines have an 'id'. This id (here "Shelly1234") is limited to Alphanumeric ASCII
        -- and must begin with a letter.
        Shelly1234 = {
            -- Actors are modular pieces of code that are loaded as required. The "Shelly" module will send
            -- activation signals to a shelly switched power socket over MQTT
            module = "Shelly",
            -- Actors can have arbitrary parameters passed to them, varying by actor module.
            params = {
                -- For Shelly you can configure the MQTT topic segment it uses. Shellies listen to a specific topic
                -- containing their name (which is usually of the form "shelly_<id>" but can be changed).
                -- If you do not configure a topic here the actor will use it's 'id' (in this case "Shelly1234").
                topic = "Topic1234"
            }
        },

        Bash = {
            -- The "Process" module runs a given script or command on state change.
            -- bffh invoces the given cmd as `$ ${cmd} ${args} ${id} ${state}` so e.g. as
            -- `$ ./examples/actor.sh your ad could be here Bash inuse`
            module = "Process",
            params = {
                -- which is configured by the (required) 'cmd' parameter. Paths are relative to PWD of bffh. Systemd
                -- and similar process managers may change this PWD so it's usually the most future-proof to use
                -- absolute paths.
                cmd = "./examples/actor.sh",
                -- You can pass static args in here, these will be passed to every invocation of the command by this actor.
                -- args passed here are split by whitespace, so these here will be passed as 5 separate arguments
                args = "your ad could be here"
            }
        },

        DoorControl1 = {
            -- This actor calls the actor.py script in examples/
            -- It gets passed it's own name, so you can have several actors
            -- from the same script.
            -- If you need to pass more arguments to the command you can use the `args` key in
            -- `params` as is done with the actor `Bash`
            module = "Process",
            params = { cmd = "./examples/actor.py", }
        },
        DoorControl2 = {
            module = "Process",
            params = { cmd = "./examples/actor.py", }
        },
        DoorControl3 = {
            -- This is an example for how it looks like if an actor is misconfigured.
            -- the actor.py doesn't know anything about DoorControl3 and, if this actor is enabled,
            -- will return with an error showing up in the server logs.
            module = "Process",
            params = { cmd = "./examples/actor.py", }
        },

        Bash2 = { module = "Process", params = { cmd = "./examples/actor.sh" , args = "this is a different one" }},
        FailBash = { module = "Process", params = { cmd = "./examples/fail-actor.sh" }}
    },

    -- Linkng up machines to actors
    -- Actors need to be connected to machines to be useful. A machine can be connected to multiple actors, but one
    -- actor can only be connected to one machine.
    actor_connections = [
        { machine = "Testmachine", actor = "DoorControl1" },
        { machine = "Another", actor = "Bash" },
        { machine = "Yetmore", actor = "Bash2" },
        { machine = "Yetmore", actor = "FailBash"}
    ],

    -- Initiators are configured almost the same way as Actors, refer to actor documentation for more details
    -- The below '{=}' is what you need if you want to define *no* initiators at all and only use the API with apps
    -- to let people use machines.
    -- initiators = {=},
    -- The "Dummy" initiator will try to use and return a machine as the given user every few seconds. It's good to
    -- test your system but will spam your log so is disabled by default.
    initiators = { Initiator = { module = "Dummy", params = { uid = "Testuser" } } },

    -- Linking up machines to initiators. Similar to actors a machine can have several initiators assigned but an
    -- initiator can only be assigned to one machine.
    -- The below is once again how you have to define *no* initiators.
    --init_connections = [] : List { machine : Text, initiator : Text }
     init_connections = [{ machine = "Testmachine", initiator = "Initiator" }]
}
