let Map = https://prelude.dhall-lang.org/v20.2.0/Map/Type
  sha256:210c7a9eba71efbb0f7a66b3dcf8b9d3976ffc2bc0e907aadfb6aa29c333e8ed

let Machine =
  { Type =
    { description : Optional Text
    , manage      : Text
    , write       : Text
    , read        : Text
    , disclose    : Text
    }
  , default = { description = None }
  }

in Machine
