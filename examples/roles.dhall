{ testrole = 
  { permissions = [ "lab.test.*" ] }
, somerole = 
  { parents = ["testparent"]
  , permissions = [ "lab.some.admin" ]
  }
, testparent = 
  { permissions = 
    [ "lab.some.write"
    , "lab.some.read"
    , "lab.some.disclose"
    ]
  }
}
