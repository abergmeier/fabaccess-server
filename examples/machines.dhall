{ Testmachine =
  { description = Some "A test machine"
  , name = "Textmachine"

  , manage = "lab.test.admin"
  , read = "lab.test.read"
  , write = "lab.test.write"
  , disclose = "lab.test.read"
  }
, Another =
  { description = Some "Another test machine"
  , name = "Another"

  , disclose = "lab.test.read"
  , manage = "lab.test.admin"
  , read = "lab.test.read"
  , write = "lab.test.write" 
  },
  Yetmore =
  { description = Some "Yet more test machines"
  , name = "Yetmore"

  , disclose = "lab.test.read"
  , manage = "lab.test.admin"
  , read = "lab.test.read"
  , write = "lab.test.write" 
  }
}
