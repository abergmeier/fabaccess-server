use std::collections::HashMap;
use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::pry;
use once_cell::sync::Lazy;

struct Locales {
    map: HashMap<&'static str, HashMap<&'static str, &'static str>>
}

impl Locales {
    pub fn get(&self, lang: &str, msg: &str)
        -> Option<(&'static str, &'static str)>
    {
        self.map.get(msg).and_then(|map| {
            map.get_key_value(lang).map(|(k,v)| (*k, *v))
        })
    }

    pub fn available(&self, _msg: &str) -> &[&'static str] {
        &[]
    }
}

static LANG: Lazy<Locales> = Lazy::new(|| {
    Locales { map: HashMap::new() }
});

struct L10NString {
    msg: &'static str,
}

/*
impl l10n::Server for L10NString {
    fn get(&mut self, params: l10n::GetParams, mut results: l10n::GetResults)
        -> Promise<(), Error>
    {
        let lang = pry!(pry!(params.get()).get_lang());

        if let Some((lang, content)) = LANG.get(lang, &self.msg) {
            let mut builder = results.get();
            builder.set_lang(lang);
            builder.set_content(content);
        }

        Promise::ok(())
    }

    fn available(&mut self, _: l10n::AvailableParams, mut results: l10n::AvailableResults)
        -> Promise<(), Error>
    {
        let langs = LANG.available(self.msg);
        let builder = results.get();
        let mut lb = builder.init_langs(langs.len() as u32);
        for (n, lang) in langs.into_iter().enumerate() {
            lb.reborrow().set(n as u32, *lang);
        }

        Promise::ok(())
    }
}
 */